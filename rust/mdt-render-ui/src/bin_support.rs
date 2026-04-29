use std::fmt::Display;
use std::fs;
use std::path::Path;
use std::str::FromStr;

const DEFAULT_WORLD_STREAM_HEX: &str =
    include_str!("../../../fixtures/world-streams/archipelago-6567-world-stream.hex");

pub fn read_world_stream_bytes(path: Option<&Path>) -> Result<Vec<u8>, String> {
    let world_stream_hex = match path {
        Some(path) => fs::read_to_string(path).map_err(|err| err.to_string())?,
        None => DEFAULT_WORLD_STREAM_HEX.to_string(),
    };
    decode_hex(&world_stream_hex)
}

pub fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
    let cleaned = text
        .chars()
        .filter(|ch| !ch.is_ascii_whitespace())
        .collect::<String>();
    if cleaned.len() % 2 != 0 {
        return Err("hex input length must be even".to_string());
    }

    cleaned
        .as_bytes()
        .chunks(2)
        .enumerate()
        .map(|(pair_index, chunk)| {
            let pair = std::str::from_utf8(chunk)
                .map_err(|err| format!("invalid hex at byte-pair {pair_index}: {err}"))?;
            u8::from_str_radix(pair, 16)
                .map_err(|err| format!("invalid hex at byte-pair {pair_index} ({pair}): {err}"))
        })
        .collect()
}

pub fn next_arg_value<I>(pending: &mut I, flag: &str) -> Result<String, String>
where
    I: Iterator<Item = String>,
{
    pending
        .next()
        .ok_or_else(|| format!("missing value for {flag}"))
}

pub fn parse_view_dimensions(value: &str) -> Result<(usize, usize), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err("invalid --max-view-tiles, expected <width:height>".to_string());
    }
    Ok((
        parse_positive_usize("--max-view-tiles width", parts[0])?,
        parse_positive_usize("--max-view-tiles height", parts[1])?,
    ))
}

pub fn parse_positive_u64(flag: &str, value: &str) -> Result<u64, String> {
    parse_positive_integer(flag, value)
}

pub fn parse_positive_u32(flag: &str, value: &str) -> Result<u32, String> {
    parse_positive_integer(flag, value)
}

pub fn parse_positive_usize(flag: &str, value: &str) -> Result<usize, String> {
    parse_positive_integer(flag, value)
}

pub fn parse_finite_f32(flag: &str, value: &str) -> Result<f32, String> {
    let parsed = parse_with_flag::<f32>(flag, value)?;
    if !parsed.is_finite() {
        return Err(format!("invalid {flag}: must be finite"));
    }
    Ok(parsed)
}

fn parse_positive_integer<T>(flag: &str, value: &str) -> Result<T, String>
where
    T: Default + PartialEq + FromStr,
    T::Err: Display,
{
    let parsed = parse_with_flag::<T>(flag, value)?;
    if parsed == T::default() {
        return Err(format!("invalid {flag}: must be greater than 0"));
    }
    Ok(parsed)
}

fn parse_with_flag<T>(flag: &str, value: &str) -> Result<T, String>
where
    T: FromStr,
    T::Err: Display,
{
    value
        .parse::<T>()
        .map_err(|err| format!("invalid {flag}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::{
        decode_hex, next_arg_value, parse_finite_f32, parse_positive_u32, parse_positive_u64,
        parse_positive_usize, parse_view_dimensions, read_world_stream_bytes,
    };
    use std::fmt::Debug;

    fn assert_ok_eq<T>(result: Result<T, String>, expected: T)
    where
        T: PartialEq + Debug,
    {
        assert_eq!(result.unwrap(), expected);
    }

    fn assert_err_eq<T: Debug>(result: Result<T, String>, expected: &str) {
        assert_eq!(result.unwrap_err(), expected);
    }

    fn assert_err_contains<T: Debug>(result: Result<T, String>, fragments: &[&str]) {
        let err = result.unwrap_err();
        for fragment in fragments {
            assert!(
                err.contains(fragment),
                "expected `{err}` to contain `{fragment}`"
            );
        }
    }

    #[test]
    fn decode_hex_ignores_ascii_whitespace() {
        assert_ok_eq(decode_hex("0a 0b\n0c\t0d"), vec![10, 11, 12, 13]);
    }

    #[test]
    fn decode_hex_reports_invalid_pair_index() {
        assert_err_contains(decode_hex("0a zz 0c"), &["byte-pair 1", "zz"]);
    }

    #[test]
    fn read_world_stream_bytes_uses_default_fixture() {
        let bytes = read_world_stream_bytes(None).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn next_arg_value_reports_missing_value() {
        let mut pending = Vec::<String>::new().into_iter();

        assert_err_eq(
            next_arg_value(&mut pending, "--fps"),
            "missing value for --fps",
        );
    }

    #[test]
    fn parse_view_dimensions_requires_width_and_height() {
        assert_err_eq(
            parse_view_dimensions("48"),
            "invalid --max-view-tiles, expected <width:height>",
        );
        assert_err_eq(
            parse_view_dimensions("0:24"),
            "invalid --max-view-tiles width: must be greater than 0",
        );
    }

    #[test]
    fn parse_positive_integer_helpers_reject_zero() {
        assert_ok_eq(parse_positive_u64("--frames", "10"), 10);
        assert_ok_eq(parse_positive_u32("--fps", "20"), 20);
        assert_ok_eq(parse_positive_usize("--tile-pixels", "12"), 12);
        assert_err_eq(
            parse_positive_u64("--frames", "0"),
            "invalid --frames: must be greater than 0",
        );
    }

    #[test]
    fn parse_finite_f32_rejects_nonfinite_values() {
        assert_ok_eq(parse_finite_f32("--player-x", "1.5"), 1.5);
        assert_err_eq(
            parse_finite_f32("--player-x", "NaN"),
            "invalid --player-x: must be finite",
        );
    }
}
