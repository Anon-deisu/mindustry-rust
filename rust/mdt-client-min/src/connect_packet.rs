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
        require_non_empty_connect_field("versionType", &self.version_type)?;
        require_non_empty_connect_field("name", &self.name)?;
        require_non_empty_connect_field("locale", &self.locale)?;
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
    strict_connect_build(VERSION_PROPERTIES)
        .expect("embedded version.properties must contain a valid integer number/build")
}

pub fn default_connect_version_type() -> &'static str {
    strict_connect_version_type(VERSION_PROPERTIES)
        .expect("embedded version.properties must contain a non-empty type")
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectVersionPropertiesError {
    MissingKey(&'static str),
    EmptyValue(&'static str),
    InvalidUtf8(&'static str),
    InvalidBuildNumber,
}

fn strict_connect_build(version_properties: &[u8]) -> Result<i32, ConnectVersionPropertiesError> {
    let number = version_properties_value_from_bytes(version_properties, "number")?;
    if let Some(number) = number {
        return parse_connect_build_number(number, "number");
    }

    let build = strict_version_properties_value(version_properties, "build")?;
    parse_connect_build_number(build, "build")
}

fn strict_connect_version_type<'a>(
    version_properties: &'a [u8],
) -> Result<&'a str, ConnectVersionPropertiesError> {
    strict_version_properties_value(version_properties, "type")
}

fn strict_version_properties_value<'a>(
    version_properties: &'a [u8],
    key: &'static str,
) -> Result<&'a str, ConnectVersionPropertiesError> {
    version_properties_value_from_bytes(version_properties, key)?
        .ok_or(ConnectVersionPropertiesError::MissingKey(key))
        .and_then(|value| {
            if value.is_empty() {
                Err(ConnectVersionPropertiesError::EmptyValue(key))
            } else {
                Ok(value)
            }
        })
}

fn parse_connect_build_number(
    value: &str,
    key: &'static str,
) -> Result<i32, ConnectVersionPropertiesError> {
    if value.is_empty() {
        return Err(ConnectVersionPropertiesError::EmptyValue(key));
    }
    value
        .parse::<i32>()
        .map_err(|_| ConnectVersionPropertiesError::InvalidBuildNumber)
}

fn version_properties_value_from_bytes<'a>(
    version_properties: &'a [u8],
    key: &'static str,
) -> Result<Option<&'a str>, ConnectVersionPropertiesError> {
    let key_bytes = key.as_bytes();
    for line in version_properties.split(|byte| *byte == b'\n') {
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
        return std::str::from_utf8(value)
            .map(Some)
            .map_err(|_| ConnectVersionPropertiesError::InvalidUtf8(key));
    }
    Ok(None)
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
    if effective_len == 0 {
        if cleaned.len() == 4 {
            return Ok(Vec::new());
        }
        return Err(ConnectPacketEncodeError::InvalidBase64Length(cleaned.len()));
    }
    let padding_len = cleaned.len() - effective_len;
    if padding_len > 2 {
        return Err(ConnectPacketEncodeError::InvalidBase64Length(cleaned.len()));
    }
    match padding_len {
        0 if effective_len % 4 != 0 => {
            return Err(ConnectPacketEncodeError::InvalidBase64Length(cleaned.len()));
        }
        1 if effective_len % 4 != 3 => {
            return Err(ConnectPacketEncodeError::InvalidBase64Length(cleaned.len()));
        }
        2 if effective_len % 4 != 2 => {
            return Err(ConnectPacketEncodeError::InvalidBase64Length(cleaned.len()));
        }
        _ => {}
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

    fn wire_string_len(value: &str) -> usize {
        1 + 2 + value.len()
    }

    fn ascii_string(len: usize) -> String {
        "a".repeat(len)
    }

    #[test]
    fn encodes_java_golden_connect_packet_bytes() {
        let mut expected = decode_hex_text(include_str!(
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
        let raw_uuid = decode_base64(&spec.uuid).unwrap();
        let crc = (crc32(&raw_uuid) as u64).to_be_bytes();
        let uuid_offset = 4
            + wire_string_len(&spec.version_type)
            + wire_string_len(&spec.name)
            + wire_string_len(&spec.locale)
            + wire_string_len(&spec.usid);
        expected.splice(
            uuid_offset..uuid_offset + 24,
            raw_uuid.into_iter().chain(crc),
        );

        assert_eq!(actual, expected);
    }

    #[test]
    fn generated_identity_roundtrips_to_eight_bytes() {
        let encoded = generate_connect_identity_base64();
        let decoded = decode_base64(&encoded).unwrap();

        assert_eq!(decoded.len(), 8);
    }

    #[test]
    fn decode_base64_rejects_invalid_character() {
        let err = decode_base64("AA*A").unwrap_err();

        assert_eq!(
            err,
            ConnectPacketEncodeError::InvalidBase64Char {
                ch: '*',
                index: 2,
            }
        );
    }

    #[test]
    fn decode_base64_rejects_invalid_length() {
        let err = decode_base64("AAAAA").unwrap_err();

        assert_eq!(err, ConnectPacketEncodeError::InvalidBase64Length(5));
    }

    #[test]
    fn decode_base64_rejects_internal_padding() {
        let err = decode_base64("AA=A").unwrap_err();

        assert_eq!(
            err,
            ConnectPacketEncodeError::InvalidBase64Char {
                ch: '=',
                index: 2,
            }
        );
    }

    #[test]
    fn decode_base64_rejects_invalid_padding_shape() {
        assert_eq!(
            decode_base64("A===").unwrap_err(),
            ConnectPacketEncodeError::InvalidBase64Length(4)
        );
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
    fn encode_payload_rejects_empty_version_type_preflight() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.version_type = "   ".to_string();

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(err, ConnectPacketEncodeError::EmptyField("versionType"));
    }

    #[test]
    fn encode_payload_rejects_empty_name_preflight() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.name = "   ".to_string();

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(err, ConnectPacketEncodeError::EmptyField("name"));
    }

    #[test]
    fn encode_payload_rejects_empty_locale_preflight() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.locale = "   ".to_string();

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(err, ConnectPacketEncodeError::EmptyField("locale"));
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
    fn encode_payload_accepts_non_empty_variable_uuid_length() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.uuid = "AAECAw==".to_string();

        let encoded = spec.encode_payload().unwrap();
        let raw_uuid = decode_base64(&spec.uuid).unwrap();
        let uuid_offset = 4
            + wire_string_len(&spec.version_type)
            + wire_string_len(&spec.name)
            + wire_string_len(&spec.locale)
            + wire_string_len(&spec.usid);

        assert_eq!(&encoded[uuid_offset..uuid_offset + raw_uuid.len()], raw_uuid.as_slice());
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
    fn encode_payload_rejects_too_many_mods_boundary() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.mods = vec![String::from("mod-a:1"); 256];

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(err, ConnectPacketEncodeError::TooManyMods(256));
    }

    #[test]
    fn encode_payload_rejects_string_too_long_boundary() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.name = ascii_string(65_536);

        let err = spec.encode_payload().unwrap_err();
        assert_eq!(
            err,
            ConnectPacketEncodeError::StringTooLong {
                field: "name",
                utf_len: 65_536,
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

    #[test]
    fn connect_compatibility_warning_code_as_str_maps_all_variants_stably() {
        assert_eq!(
            ConnectCompatibilityWarningCode::BuildUnknown.as_str(),
            "build_unknown"
        );
        assert_eq!(
            ConnectCompatibilityWarningCode::VersionTypeCustomLike.as_str(),
            "version_type_custom_like"
        );
    }

    #[test]
    fn strict_connect_build_rejects_missing_or_invalid_build_metadata() {
        assert_eq!(
            strict_connect_build(b"number = 8\nbuild = custom build\ntype = official\n"),
            Ok(8)
        );
        assert_eq!(
            strict_connect_build(b"type = official\n"),
            Err(ConnectVersionPropertiesError::MissingKey("build"))
        );
        assert_eq!(
            strict_connect_build(b"number = nope\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
        assert_eq!(
            strict_connect_build(b"number = \xff\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidUtf8("number"))
        );
        assert_eq!(
            strict_connect_build(b"build = 156\ntype = official\n"),
            Ok(156)
        );
        assert_eq!(
            strict_connect_build(b"build = nope\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
        assert_eq!(
            strict_connect_build(b"build = \xff\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidUtf8("build"))
        );
    }

    #[test]
    fn strict_connect_version_type_rejects_missing_or_empty_type_metadata() {
        assert_eq!(
            strict_connect_version_type(b"build = 146\n"),
            Err(ConnectVersionPropertiesError::MissingKey("type"))
        );
        assert_eq!(
            strict_connect_version_type(b"build = 146\ntype =   \n"),
            Err(ConnectVersionPropertiesError::EmptyValue("type"))
        );
        assert_eq!(
            strict_connect_version_type(b"build = 146\ntype = \xff\n"),
            Err(ConnectVersionPropertiesError::InvalidUtf8("type"))
        );
    }

    #[test]
    fn parse_connect_build_number_handles_success_and_failure_cases() {
        assert_eq!(
            parse_connect_build_number("146", "build"),
            Ok(146)
        );
        assert_eq!(
            parse_connect_build_number("-7", "build"),
            Ok(-7)
        );
        assert_eq!(
            parse_connect_build_number("", "build"),
            Err(ConnectVersionPropertiesError::EmptyValue("build"))
        );
        assert_eq!(
            parse_connect_build_number("nope", "build"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
    }

    #[test]
    fn trim_ascii_strips_ascii_whitespace_only() {
        assert_eq!(trim_ascii(b"\t  hello \r\n"), b"hello");

        let padded = " \u{00a0}héllo\u{00a0} ".as_bytes();
        assert_eq!(trim_ascii(padded), " héllo ".as_bytes());
    }
}
