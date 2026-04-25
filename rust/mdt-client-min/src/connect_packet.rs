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

        if is_custom_like_version_type(&self.version_type) {
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
        validate_connect_mod_entries(&self.mods)?;

        let raw_uuid = decode_base64(&self.uuid)?;
        if raw_uuid.is_empty() {
            return Err(ConnectPacketEncodeError::InvalidUuidLength(0));
        }
        Ok(raw_uuid)
    }
}

fn is_custom_like_version_type(version_type: &str) -> bool {
    let normalized_version_type = version_type.trim();
    normalized_version_type.eq_ignore_ascii_case("custom")
        || normalized_version_type.eq_ignore_ascii_case("custom build")
}

fn validate_connect_mod_entries(mods: &[String]) -> Result<(), ConnectPacketEncodeError> {
    for (index, entry) in mods.iter().enumerate() {
        if entry.trim().is_empty() {
            return Err(ConnectPacketEncodeError::InvalidModEntry {
                index,
                reason: "must not be empty",
            });
        }
    }

    Ok(())
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
    if connect_field_is_blank(value) {
        return Err(ConnectPacketEncodeError::EmptyField(field));
    }
    Ok(())
}

fn connect_field_is_blank(value: &str) -> bool {
    value.trim().is_empty()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConnectVersionPropertiesError {
    MissingKey(&'static str),
    EmptyValue(&'static str),
    InvalidUtf8(&'static str),
    InvalidBuildNumber,
}

fn strict_connect_build(version_properties: &[u8]) -> Result<i32, ConnectVersionPropertiesError> {
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

    let (build, revision) = value
        .split_once('.')
        .map_or((value, None), |(build, revision)| (build, Some(revision)));
    if build.is_empty() {
        return Err(ConnectVersionPropertiesError::InvalidBuildNumber);
    }
    validate_optional_build_revision(revision)?;

    build
        .parse::<i32>()
        .map_err(|_| ConnectVersionPropertiesError::InvalidBuildNumber)
}

fn validate_optional_build_revision(
    revision: Option<&str>,
) -> Result<(), ConnectVersionPropertiesError> {
    if revision
        .is_some_and(|revision| revision.is_empty() || revision.parse::<i32>().is_err())
    {
        return Err(ConnectVersionPropertiesError::InvalidBuildNumber);
    }
    Ok(())
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
        match modified_utf8_unit_len(unit) {
            1 => out.push(unit as u8),
            2 => {
                out.push((0xc0 | ((unit >> 6) & 0x1f)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
            3 => {
                out.push((0xe0 | ((unit >> 12) & 0x0f)) as u8);
                out.push((0x80 | ((unit >> 6) & 0x3f)) as u8);
                out.push((0x80 | (unit & 0x3f)) as u8);
            }
            _ => unreachable!("modified utf-8 unit length is always 1, 2, or 3"),
        }
    }

    Ok(())
}

fn modified_utf8_len(value: &str) -> usize {
    value.encode_utf16().map(modified_utf8_unit_len).sum()
}

fn modified_utf8_unit_len(unit: u16) -> usize {
    match unit {
        0x0001..=0x007f => 1,
        0x0000 | 0x0080..=0x07ff => 2,
        _ => 3,
    }
}

fn decode_base64(input: &str) -> Result<Vec<u8>, ConnectPacketEncodeError> {
    let cleaned = strip_base64_whitespace(input);
    let effective_len = decode_base64_effective_len(&cleaned)?;

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

fn decode_base64_effective_len(cleaned: &[char]) -> Result<usize, ConnectPacketEncodeError> {
    if cleaned.len() % 4 != 0 {
        return Err(ConnectPacketEncodeError::InvalidBase64Length(cleaned.len()));
    }

    let mut effective_len = cleaned.len();
    while effective_len > 0 && cleaned[effective_len - 1] == '=' {
        effective_len -= 1;
    }
    if effective_len == 0 {
        if cleaned.len() == 4 {
            return Ok(0);
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
        _ => Ok(effective_len),
    }
}

fn strip_base64_whitespace(input: &str) -> Vec<char> {
    input.chars().filter(|ch| !ch.is_whitespace()).collect()
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
    fn decode_base64_accepts_all_padding_quad_as_empty() {
        assert_eq!(decode_base64("====").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn decode_base64_ignores_whitespace_between_quads() {
        assert_eq!(decode_base64(" Z m\n8= \t").unwrap(), b"fo".to_vec());
    }

    #[test]
    fn decode_base64_effective_len_tracks_padding_boundaries() {
        fn chars(text: &str) -> Vec<char> {
            text.chars().collect()
        }

        assert_eq!(decode_base64_effective_len(&chars("TQ==")).unwrap(), 2);
        assert_eq!(decode_base64_effective_len(&chars("TWE=")).unwrap(), 3);
        assert_eq!(decode_base64_effective_len(&chars("TWFu")).unwrap(), 4);
        assert_eq!(decode_base64_effective_len(&chars("====")).unwrap(), 0);
        assert_eq!(
            decode_base64_effective_len(&chars("A===")).unwrap_err(),
            ConnectPacketEncodeError::InvalidBase64Length(4)
        );
        assert_eq!(
            decode_base64_effective_len(&chars("")).unwrap_err(),
            ConnectPacketEncodeError::InvalidBase64Length(0)
        );
    }

    #[test]
    fn strip_base64_whitespace_removes_all_unicode_whitespace() {
        assert_eq!(strip_base64_whitespace(" A\u{00a0}B\tC\nD\rE\u{2003}F "), vec!['A', 'B', 'C', 'D', 'E', 'F']);
    }

    #[test]
    fn decode_base64_accepts_urlsafe_alphabet_variants() {
        assert_eq!(decode_base64("AA-_").unwrap(), vec![0, 15, 191]);
    }

    #[test]
    fn decode_base64_value_accepts_alphabet_edges_and_rejects_neighboring_punctuation() {
        assert_eq!(decode_base64_value('A', 0), Ok(0));
        assert_eq!(decode_base64_value('Z', 1), Ok(25));
        assert_eq!(decode_base64_value('a', 2), Ok(26));
        assert_eq!(decode_base64_value('z', 3), Ok(51));
        assert_eq!(decode_base64_value('0', 4), Ok(52));
        assert_eq!(decode_base64_value('9', 5), Ok(61));
        assert_eq!(decode_base64_value('+', 6), Ok(62));
        assert_eq!(decode_base64_value('-', 7), Ok(62));
        assert_eq!(decode_base64_value('/', 8), Ok(63));
        assert_eq!(decode_base64_value('_', 9), Ok(63));
        assert_eq!(
            decode_base64_value('.', 10),
            Err(ConnectPacketEncodeError::InvalidBase64Char { ch: '.', index: 10 })
        );
    }

    #[test]
    fn decode_base64_rejects_padding_in_second_slot() {
        assert_eq!(
            decode_base64("A=AA").unwrap_err(),
            ConnectPacketEncodeError::InvalidBase64Char { ch: '=', index: 1 }
        );
    }

    #[test]
    fn encode_base64_encodes_short_inputs_with_padding() {
        assert_eq!(encode_base64(b""), "");
        assert_eq!(encode_base64(b"f"), "Zg==");
        assert_eq!(encode_base64(b"fo"), "Zm8=");
    }

    #[test]
    fn encode_base64_preserves_exact_three_byte_chunks_without_padding() {
        assert_eq!(encode_base64(b"foo"), "Zm9v");
        assert_eq!(encode_base64(b"foobar"), "Zm9vYmFy");
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
    fn connect_field_is_blank_treats_whitespace_only_values_as_blank() {
        assert!(connect_field_is_blank(""));
        assert!(connect_field_is_blank("   "));
        assert!(connect_field_is_blank("\t\r\n"));
        assert!(!connect_field_is_blank("x"));
    }

    #[test]
    fn require_non_empty_connect_field_rejects_blank_values_with_field_name() {
        assert_eq!(
            require_non_empty_connect_field("locale", "  "),
            Err(ConnectPacketEncodeError::EmptyField("locale"))
        );
    }

    #[test]
    fn require_non_empty_connect_field_accepts_non_blank_values() {
        assert_eq!(require_non_empty_connect_field("name", "  client  "), Ok(()));
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
    fn validate_connect_mod_entries_rejects_blank_entries() {
        assert_eq!(
            validate_connect_mod_entries(&["mod-a:1".to_string(), "  ".to_string()]),
            Err(ConnectPacketEncodeError::InvalidModEntry {
                index: 1,
                reason: "must not be empty",
            })
        );
    }

    #[test]
    fn validate_connect_mod_entries_and_encode_payload_reject_first_blank_mod_entry() {
        let mods = vec!["   ".to_string(), "mod-b:2".to_string()];

        assert_eq!(
            validate_connect_mod_entries(&mods),
            Err(ConnectPacketEncodeError::InvalidModEntry {
                index: 0,
                reason: "must not be empty",
            })
        );

        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.mods = mods;

        assert_eq!(
            spec.encode_payload().unwrap_err(),
            ConnectPacketEncodeError::InvalidModEntry {
                index: 0,
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
    fn encode_payload_accepts_maximum_mod_count_boundary() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.mods = vec![String::from("mod-a:1"); 255];

        let encoded = spec.encode_payload().unwrap();
        let raw_uuid = decode_base64(&spec.uuid).unwrap();
        let mod_count_offset = 4
            + wire_string_len(&spec.version_type)
            + wire_string_len(&spec.name)
            + wire_string_len(&spec.locale)
            + wire_string_len(&spec.usid)
            + raw_uuid.len()
            + 8
            + 1
            + 4;

        assert_eq!(encoded[mod_count_offset], u8::MAX);
    }

    #[test]
    fn encode_payload_accepts_maximum_string_length_boundary() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.name = ascii_string(u16::MAX as usize);

        let encoded = spec.encode_payload().unwrap();
        let name_offset = 4 + wire_string_len(&spec.version_type);

        assert_eq!(encoded[name_offset], 1);
        assert_eq!(
            u16::from_be_bytes([encoded[name_offset + 1], encoded[name_offset + 2]]),
            u16::MAX
        );
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
    fn compatibility_warnings_return_both_codes_in_stable_order() {
        let mut spec = ConnectPacketSpec::new_default("en_US");
        spec.version = -1;
        spec.version_type = " custom build ".to_string();

        let warnings = spec
            .compatibility_warnings()
            .iter()
            .map(|warning| warning.code)
            .collect::<Vec<_>>();

        assert_eq!(
            warnings,
            vec![
                ConnectCompatibilityWarningCode::BuildUnknown,
                ConnectCompatibilityWarningCode::VersionTypeCustomLike,
            ]
        );
    }

    #[test]
    fn is_custom_like_version_type_trims_and_matches_known_custom_labels() {
        assert!(is_custom_like_version_type("custom"));
        assert!(is_custom_like_version_type("  CUSTOM BUILD  "));
        assert!(!is_custom_like_version_type("official"));
        assert!(!is_custom_like_version_type("release"));
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
            strict_connect_build(b"number = 8\nbuild = 157.2\ntype = official\n"),
            Ok(157)
        );
        assert_eq!(
            strict_connect_build(b"number = 8\nbuild = 156\ntype = official\n"),
            Ok(156)
        );
        assert_eq!(
            strict_connect_build(b"number = 999\nbuild = 157\ntype = official\n"),
            Ok(157)
        );
        assert_eq!(
            strict_connect_build(b"type = official\n"),
            Err(ConnectVersionPropertiesError::MissingKey("build"))
        );
        assert_eq!(
            strict_connect_build(b"number = nope\ntype = official\n"),
            Err(ConnectVersionPropertiesError::MissingKey("build"))
        );
        assert_eq!(
            strict_connect_build(b"build = .2\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
        assert_eq!(
            strict_connect_build(b"build = 157.\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
        assert_eq!(
            strict_connect_build(b"build = 157.x\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
        assert_eq!(
            strict_connect_build(b"build = custom build\ntype = official\n"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
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
    fn strict_connect_build_prefers_first_build_entry_over_later_duplicates() {
        assert_eq!(
            strict_connect_build(b"build = 157.2\nbuild = 999\ntype = official\n"),
            Ok(157)
        );
    }

    #[test]
    fn strict_connect_build_accepts_dotted_build_with_zero_revision() {
        assert_eq!(
            strict_connect_build(b"build = 157.0\ntype = official\n"),
            Ok(157)
        );
    }

    #[test]
    fn strict_connect_build_accepts_trimmed_crlf_157_2_metadata() {
        assert_eq!(
            strict_connect_build(b"build =  157.2  \r\ntype = official\r\n"),
            Ok(157)
        );
    }

    #[test]
    fn parse_connect_build_number_rejects_invalid_revision_but_accepts_zero_revision() {
        assert_eq!(parse_connect_build_number("157.0", "build"), Ok(157));
        assert_eq!(
            parse_connect_build_number("157.", "build"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
        assert_eq!(
            parse_connect_build_number("157.x", "build"),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
    }

    #[test]
    fn embedded_version_properties_track_upstream_157_2_connect_metadata() {
        assert_eq!(default_connect_build(), 157);
        assert_eq!(default_connect_version_type(), "official");

        let spec = ConnectPacketSpec::new_default("en_US");
        assert_eq!(spec.version, 157);
        assert_eq!(spec.version_type, "official");
    }

    #[test]
    fn new_default_uses_embedded_metadata_and_stable_defaults() {
        let spec = ConnectPacketSpec::new_default("zh_CN");

        assert_eq!(spec.version, default_connect_build());
        assert_eq!(spec.version_type, default_connect_version_type());
        assert_eq!(spec.name, "mdt-client-min");
        assert_eq!(spec.locale, "zh_CN");
        assert!(!spec.usid.is_empty());
        assert!(!spec.uuid.is_empty());
        assert!(!spec.mobile);
        assert_eq!(spec.color, -1);
        assert!(spec.mods.is_empty());
        assert!(spec.compatibility_warnings().is_empty());
        assert!(spec.encode_payload().is_ok());
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
    fn version_properties_value_from_bytes_prefers_first_match_and_trims_crlf() {
        assert_eq!(
            version_properties_value_from_bytes(
                b"build = 146\r\nbuild = 999\n",
                "build"
            )
            .unwrap(),
            Some("146")
        );
        assert_eq!(
            version_properties_value_from_bytes(b"type = official\r\n", "number").unwrap(),
            None
        );
    }

    #[test]
    fn version_properties_value_from_bytes_skips_malformed_lines_and_finds_later_key() {
        assert_eq!(
            version_properties_value_from_bytes(
                b"not-a-pair\nbuild = 146\nignored = yes\n",
                "build"
            )
            .unwrap(),
            Some("146")
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
    fn parse_connect_build_number_accepts_explicit_positive_sign() {
        assert_eq!(parse_connect_build_number("+7", "build"), Ok(7));
    }

    #[test]
    fn validate_optional_build_revision_accepts_signed_numeric_revisions_and_rejects_non_numeric() {
        assert_eq!(validate_optional_build_revision(None), Ok(()));
        assert_eq!(validate_optional_build_revision(Some("0")), Ok(()));
        assert_eq!(validate_optional_build_revision(Some("-1")), Ok(()));
        assert_eq!(
            validate_optional_build_revision(Some("x")),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
        assert_eq!(
            validate_optional_build_revision(Some("")),
            Err(ConnectVersionPropertiesError::InvalidBuildNumber)
        );
    }

    #[test]
    fn strict_connect_version_type_prefers_first_entry_and_trims_ascii_whitespace() {
        assert_eq!(
            strict_connect_version_type(b"type =  custom build  \ntype = official\n"),
            Ok("custom build")
        );
    }

    #[test]
    fn trim_ascii_strips_ascii_whitespace_only() {
        assert_eq!(trim_ascii(b"\t  hello \r\n"), b"hello");

        let padded = " \t h\u{00e8}llo \r\n".as_bytes();
        assert_eq!(trim_ascii(padded), "h\u{00e8}llo".as_bytes());
    }

    #[test]
    fn modified_utf8_len_counts_nul_bmp_and_surrogate_units() {
        assert_eq!(modified_utf8_len(""), 0);
        assert_eq!(modified_utf8_len("\0"), 2);
        assert_eq!(modified_utf8_len("A"), 1);
        assert_eq!(modified_utf8_len("\u{00e8}"), 2);
        assert_eq!(modified_utf8_len("\u{0800}"), 3);
        assert_eq!(modified_utf8_len("\u{1f600}"), 6);
    }

    #[test]
    fn modified_utf8_unit_len_matches_java_modified_utf8_boundaries() {
        assert_eq!(modified_utf8_unit_len(0x0000), 2);
        assert_eq!(modified_utf8_unit_len(0x0001), 1);
        assert_eq!(modified_utf8_unit_len(0x007f), 1);
        assert_eq!(modified_utf8_unit_len(0x0080), 2);
        assert_eq!(modified_utf8_unit_len(0x07ff), 2);
        assert_eq!(modified_utf8_unit_len(0x0800), 3);
        assert_eq!(modified_utf8_unit_len(0xd83d), 3);
        assert_eq!(modified_utf8_unit_len(0xde00), 3);
    }

    #[test]
    fn write_java_modified_utf_prefixes_length_and_preserves_nul_and_astral_boundaries() {
        let mut out = Vec::new();
        write_java_modified_utf(&mut out, "field", "A\0\u{1f600}").unwrap();

        assert_eq!(
            out,
            vec![
                0x00, 0x09, 0x41, 0xc0, 0x80, 0xed, 0xa0, 0xbd, 0xed, 0xb8, 0x80,
            ]
        );
    }
}
