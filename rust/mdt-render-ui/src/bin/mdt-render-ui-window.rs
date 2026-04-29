use mdt_render_ui::{
    bin_support::{
        next_arg_value, parse_finite_f32, parse_positive_u64, parse_positive_usize,
        parse_view_dimensions,
    },
    project_scene_models_with_view_window, read_world_stream_bytes, MinifbWindowBackend,
    WindowPresenter,
};
use mdt_world::parse_world_bundle;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

fn main() -> Result<(), String> {
    match parse_args(std::env::args().skip(1))? {
        ParseOutcome::Help(usage) => {
            println!("{usage}");
            return Ok(());
        }
        ParseOutcome::Args(args) => run(args)?,
    }
    Ok(())
}

fn run(args: Args) -> Result<(), String> {
    let bytes = read_world_stream_bytes(args.world_stream_hex.as_deref())?;
    let bundle = parse_world_bundle(&bytes)?;
    let session = bundle.loaded_session()?;
    let base_player_position = args
        .player_position
        .unwrap_or_else(|| session.state().player_position());

    let backend = MinifbWindowBackend::new(args.tile_pixels, "mdt-render-ui-window");
    let mut presenter = WindowPresenter::new(backend)
        .with_max_view_tiles(args.max_view_tiles.0, args.max_view_tiles.1)
        .with_target_fps(args.target_fps());

    let start = Instant::now();
    while start.elapsed() < args.duration {
        let runtime_position = if args.animate_player {
            animated_player_position(base_player_position, start.elapsed())
        } else {
            base_player_position
        };
        let (scene, mut hud) = project_scene_models_with_view_window(
            &session,
            &args.locale,
            Some(runtime_position),
            args.max_view_tiles,
        );
        hud.fps = Some(args.target_fps() as f32);
        presenter.present_once(&scene, &hud)?;
        thread::sleep(args.frame_time);
    }

    println!("rendered window for {}ms", args.duration.as_millis());
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
    world_stream_hex: Option<PathBuf>,
    duration: Duration,
    frame_time: Duration,
    tile_pixels: usize,
    max_view_tiles: (usize, usize),
    player_position: Option<(f32, f32)>,
    animate_player: bool,
}

impl Args {
    fn target_fps(&self) -> u32 {
        let millis = self.frame_time.as_millis().max(1);
        (1000 / millis).max(1) as u32
    }
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<ParseOutcome, String> {
    let mut locale = String::from("en");
    let mut world_stream_hex = None;
    let mut duration = Duration::from_millis(4_000);
    let mut frame_time = Duration::from_millis(33);
    let mut tile_pixels = 12usize;
    let mut max_view_tiles = (64usize, 32usize);
    let mut player_x = None;
    let mut player_y = None;
    let mut animate_player = true;
    let mut locale_seen = false;
    let mut world_stream_hex_seen = false;
    let mut duration_seen = false;
    let mut frame_time_seen = false;
    let mut tile_pixels_seen = false;
    let mut max_view_tiles_seen = false;
    let mut player_x_seen = false;
    let mut player_y_seen = false;
    let mut pending = args.collect::<Vec<_>>().into_iter();

    while let Some(arg) = pending.next() {
        match arg.as_str() {
            "--locale" => {
                if locale_seen {
                    return Err("duplicate argument: --locale".to_string());
                }
                locale_seen = true;
                locale = next_arg_value(&mut pending, "--locale")?;
            }
            "--world-stream-hex" => {
                if world_stream_hex_seen {
                    return Err("duplicate argument: --world-stream-hex".to_string());
                }
                world_stream_hex_seen = true;
                world_stream_hex = Some(PathBuf::from(next_arg_value(
                    &mut pending,
                    "--world-stream-hex",
                )?));
            }
            "--duration-ms" => {
                if duration_seen {
                    return Err("duplicate argument: --duration-ms".to_string());
                }
                duration_seen = true;
                duration = Duration::from_millis(parse_positive_u64(
                    "--duration-ms",
                    &next_arg_value(&mut pending, "--duration-ms")?,
                )?);
            }
            "--frame-ms" => {
                if frame_time_seen {
                    return Err("duplicate argument: --frame-ms".to_string());
                }
                frame_time_seen = true;
                frame_time = Duration::from_millis(parse_positive_u64(
                    "--frame-ms",
                    &next_arg_value(&mut pending, "--frame-ms")?,
                )?);
            }
            "--max-view-tiles" => {
                if max_view_tiles_seen {
                    return Err("duplicate argument: --max-view-tiles".to_string());
                }
                max_view_tiles_seen = true;
                max_view_tiles =
                    parse_view_dimensions(&next_arg_value(&mut pending, "--max-view-tiles")?)?;
            }
            "--tile-pixels" => {
                if tile_pixels_seen {
                    return Err("duplicate argument: --tile-pixels".to_string());
                }
                tile_pixels_seen = true;
                tile_pixels = parse_positive_usize(
                    "--tile-pixels",
                    &next_arg_value(&mut pending, "--tile-pixels")?,
                )?;
            }
            "--player-x" => {
                if player_x_seen {
                    return Err("duplicate argument: --player-x".to_string());
                }
                player_x_seen = true;
                player_x = Some(parse_finite_f32(
                    "--player-x",
                    &next_arg_value(&mut pending, "--player-x")?,
                )?);
            }
            "--player-y" => {
                if player_y_seen {
                    return Err("duplicate argument: --player-y".to_string());
                }
                player_y_seen = true;
                player_y = Some(parse_finite_f32(
                    "--player-y",
                    &next_arg_value(&mut pending, "--player-y")?,
                )?);
            }
            "--no-animate-player" => animate_player = false,
            "--help" | "-h" => return Ok(ParseOutcome::Help(usage())),
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
    }

    let player_position = match (player_x, player_y) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => return Err("both --player-x and --player-y are required".to_string()),
    };

    Ok(ParseOutcome::Args(Args {
        locale,
        world_stream_hex,
        duration,
        frame_time,
        tile_pixels,
        max_view_tiles,
        player_position,
        animate_player,
    }))
}

fn usage() -> String {
    "Usage: mdt-render-ui-window [--locale <locale>] [--world-stream-hex <path>] [--duration-ms <ms>] [--frame-ms <ms>] [--tile-pixels <n>] [--max-view-tiles <width:height>] [--player-x <f32> --player-y <f32>] [--no-animate-player]".to_string()
}

fn animated_player_position(origin: (f32, f32), elapsed: Duration) -> (f32, f32) {
    let t = elapsed.as_secs_f32();
    (
        origin.0 + (t * 2.5).sin() * 16.0,
        origin.1 + (t * 1.5).cos() * 12.0,
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Args, ParseOutcome};
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn parse_args_accepts_window_configuration() {
        let args = expect_args(parse_ok(&[
            "--locale",
            "fr",
            "--world-stream-hex",
            "sample.hex",
            "--duration-ms",
            "2500",
            "--frame-ms",
            "20",
            "--tile-pixels",
            "10",
            "--max-view-tiles",
            "48:24",
            "--player-x",
            "32",
            "--player-y",
            "48",
            "--no-animate-player",
        ]));

        assert_eq!(
            args,
            Args {
                locale: "fr".to_string(),
                world_stream_hex: Some(PathBuf::from("sample.hex")),
                duration: Duration::from_millis(2500),
                frame_time: Duration::from_millis(20),
                tile_pixels: 10,
                max_view_tiles: (48, 24),
                player_position: Some((32.0, 48.0)),
                animate_player: false,
            }
        );
    }

    #[test]
    fn parse_args_help_is_not_an_error() {
        assert_help(&["--help"]);
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
    fn parse_args_rejects_zero_frame_ms() {
        assert_parse_error_contains(
            &["--frame-ms", "0", "--player-x", "1", "--player-y", "2"],
            "invalid --frame-ms: must be greater than 0",
        );
    }

    #[test]
    fn parse_args_rejects_zero_duration_ms() {
        assert_parse_error_contains(
            &["--duration-ms", "0", "--player-x", "1", "--player-y", "2"],
            "invalid --duration-ms: must be greater than 0",
        );
    }

    #[test]
    fn parse_args_rejects_duplicate_window_flags() {
        let cases = [
            vec!["--locale", "fr", "--locale", "de"],
            vec!["--world-stream-hex", "a.hex", "--world-stream-hex", "b.hex"],
            vec!["--duration-ms", "2500", "--duration-ms", "3000"],
            vec!["--frame-ms", "20", "--frame-ms", "25"],
            vec!["--tile-pixels", "10", "--tile-pixels", "12"],
            vec!["--max-view-tiles", "48:24", "--max-view-tiles", "32:16"],
            vec!["--player-x", "1", "--player-x", "2", "--player-y", "3"],
            vec!["--player-y", "1", "--player-x", "2", "--player-y", "3"],
        ];

        for case in cases {
            assert_parse_error_starts_with(&case, "duplicate argument: ");
        }
    }

    fn argv(parts: &[&str]) -> std::vec::IntoIter<String> {
        parts
            .iter()
            .map(|part| (*part).to_string())
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn parse_ok(parts: &[&str]) -> ParseOutcome {
        parse_args(argv(parts)).unwrap()
    }

    fn expect_args(outcome: ParseOutcome) -> Args {
        match outcome {
            ParseOutcome::Args(args) => args,
            ParseOutcome::Help(_) => panic!("expected parsed args"),
        }
    }

    fn assert_help(parts: &[&str]) {
        match parse_ok(parts) {
            ParseOutcome::Help(usage) => assert!(usage.starts_with("Usage: mdt-render-ui-window")),
            ParseOutcome::Args(_) => panic!("expected help"),
        }
    }

    fn assert_parse_error_contains(parts: &[&str], expected: &str) {
        let err = parse_args(argv(parts)).unwrap_err();
        assert!(err.contains(expected), "{err}");
    }

    fn assert_parse_error_starts_with(parts: &[&str], expected_prefix: &str) {
        let err = parse_args(argv(parts)).unwrap_err();
        assert!(err.starts_with(expected_prefix), "{err}");
    }
}
