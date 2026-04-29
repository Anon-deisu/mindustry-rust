use mdt_render_ui::{
    bin_support::{
        next_arg_value, parse_finite_f32, parse_positive_u32, parse_positive_usize,
        parse_view_dimensions,
    },
    project_scene_models_with_view_window, read_world_stream_bytes, MinifbWindowBackend,
    WindowPresenter,
};
use mdt_world::parse_world_bundle;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args = match parse_args(std::env::args().skip(1))? {
        ParseOutcome::Help(usage) => {
            println!("{usage}");
            return Ok(());
        }
        ParseOutcome::Args(args) => args,
    };
    let bytes = read_world_stream_bytes(args.world_stream_hex.as_deref())?;
    let bundle = parse_world_bundle(&bytes)?;
    let session = bundle.loaded_session()?;
    let base_player_position = args
        .player_position
        .unwrap_or_else(|| session.state().player_position());
    let backend = MinifbWindowBackend::new(args.tile_pixels, "mdt-render-ui-window-demo");
    let mut presenter = WindowPresenter::new(backend).with_target_fps(args.fps);
    presenter = presenter.with_max_view_tiles(args.max_view_tiles.0, args.max_view_tiles.1);

    let stats = presenter.run_offline(args.frames, |frame_id| {
        let runtime_player_position = animated_player_position(base_player_position, frame_id);
        let (scene, mut hud) = project_scene_models_with_view_window(
            &session,
            &args.locale,
            Some(runtime_player_position),
            args.max_view_tiles,
        );
        hud.status_text = format!("{} frame={frame_id}", hud.status_text);
        hud.fps = Some(args.fps as f32);
        (scene, hud)
    })?;

    println!(
        "rendered {} frames in {}ms",
        stats.frames_rendered, stats.elapsed_ms,
    );
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
enum ParseOutcome {
    Args(Args),
    Help(String),
}

#[derive(Debug, Clone, PartialEq)]
struct Args {
    locale: String,
    frames: u64,
    fps: u32,
    tile_pixels: usize,
    max_view_tiles: (usize, usize),
    player_position: Option<(f32, f32)>,
    world_stream_hex: Option<PathBuf>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<ParseOutcome, String> {
    let mut locale = String::from("en");
    let mut frames = 120u64;
    let mut fps = 30u32;
    let mut tile_pixels = 12usize;
    let mut max_view_tiles = (64usize, 32usize);
    let mut player_x = None;
    let mut player_y = None;
    let mut world_stream_hex = None;
    let mut pending = args.collect::<Vec<_>>().into_iter();

    while let Some(arg) = pending.next() {
        match arg.as_str() {
            "--locale" => {
                locale = next_arg_value(&mut pending, "--locale")?;
            }
            "--frames" => {
                frames = parse_u64(&next_arg_value(&mut pending, "--frames")?)?;
            }
            "--fps" => {
                fps = parse_positive_u32("--fps", &next_arg_value(&mut pending, "--fps")?)?;
            }
            "--tile-pixels" => {
                tile_pixels = parse_positive_usize(
                    "--tile-pixels",
                    &next_arg_value(&mut pending, "--tile-pixels")?,
                )?;
            }
            "--max-view-tiles" => {
                max_view_tiles =
                    parse_view_dimensions(&next_arg_value(&mut pending, "--max-view-tiles")?)?;
            }
            "--player-x" => {
                player_x = Some(parse_finite_f32(
                    "--player-x",
                    &next_arg_value(&mut pending, "--player-x")?,
                )?);
            }
            "--player-y" => {
                player_y = Some(parse_finite_f32(
                    "--player-y",
                    &next_arg_value(&mut pending, "--player-y")?,
                )?);
            }
            "--world-stream-hex" => {
                world_stream_hex = Some(PathBuf::from(next_arg_value(
                    &mut pending,
                    "--world-stream-hex",
                )?));
            }
            "--help" | "-h" => {
                return Ok(ParseOutcome::Help(usage()));
            }
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        }
    }

    let player_position = match (player_x, player_y) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => return Err("both --player-x and --player-y are required".to_string()),
    };

    Ok(ParseOutcome::Args(Args {
        locale,
        frames,
        fps,
        tile_pixels,
        max_view_tiles,
        player_position,
        world_stream_hex,
    }))
}

fn animated_player_position(origin: (f32, f32), frame_id: u64) -> (f32, f32) {
    let t = frame_id as f32 / 12.0;
    (origin.0 + t.sin() * 16.0, origin.1 + t.cos() * 12.0)
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|err| err.to_string())
}

fn usage() -> String {
    "Usage: mdt-render-ui-window-demo [--locale <locale>] [--frames <n>] [--fps <n>] [--tile-pixels <n>] [--max-view-tiles <width:height>] [--player-x <f32> --player-y <f32>] [--world-stream-hex <path>]".to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Args, ParseOutcome};
    use std::path::PathBuf;

    #[test]
    fn parse_args_accepts_demo_options() {
        let args = assert_parsed_args(&[
            "--locale",
            "fr",
            "--frames",
            "240",
            "--fps",
            "20",
            "--tile-pixels",
            "10",
            "--max-view-tiles",
            "48:24",
            "--player-x",
            "32",
            "--player-y",
            "48",
            "--world-stream-hex",
            "sample.hex",
        ]);

        assert_eq!(
            args,
            Args {
                locale: "fr".to_string(),
                frames: 240,
                fps: 20,
                tile_pixels: 10,
                max_view_tiles: (48, 24),
                player_position: Some((32.0, 48.0)),
                world_stream_hex: Some(PathBuf::from("sample.hex")),
            }
        );
    }

    #[test]
    fn parse_args_help_is_not_an_error() {
        let usage = assert_help(&["--help"]);
        assert!(usage.starts_with("Usage: mdt-render-ui-window-demo"));
    }

    #[test]
    fn parse_args_rejects_nonfinite_player_coords_and_zero_sizes() {
        assert_parse_error_contains(
            &["--player-x", "NaN", "--player-y", "12"],
            "invalid --player-x: must be finite",
        );
        assert_parse_error_contains(
            &["--tile-pixels", "0", "--player-x", "1", "--player-y", "2"],
            "invalid --tile-pixels: must be greater than 0",
        );
        assert_parse_error_contains(
            &["--max-view-tiles", "0:24", "--player-x", "1", "--player-y", "2"],
            "invalid --max-view-tiles width: must be greater than 0",
        );
        assert_parse_error_contains(
            &["--player-x", "inf", "--player-y", "12"],
            "invalid --player-x: must be finite",
        );
    }

    #[test]
    fn parse_args_rejects_zero_fps() {
        assert_parse_error_contains(
            &["--fps", "0", "--player-x", "1", "--player-y", "2"],
            "invalid --fps: must be greater than 0",
        );
    }

    fn argv(parts: &[&str]) -> std::vec::IntoIter<String> {
        parts
            .iter()
            .map(|part| (*part).to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn assert_parsed_args(parts: &[&str]) -> Args {
        match parse_args(argv(parts)).unwrap() {
            ParseOutcome::Args(args) => args,
            ParseOutcome::Help(_) => panic!("expected parsed args"),
        }
    }

    fn assert_help(parts: &[&str]) -> String {
        match parse_args(argv(parts)).unwrap() {
            ParseOutcome::Help(usage) => usage,
            ParseOutcome::Args(_) => panic!("expected help"),
        }
    }

    fn assert_parse_error_contains(parts: &[&str], expected: &str) {
        let err = parse_args(argv(parts)).unwrap_err();
        assert!(err.contains(expected), "expected '{expected}' in '{err}'");
    }
}
