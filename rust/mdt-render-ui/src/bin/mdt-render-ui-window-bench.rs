use mdt_render_ui::{
    bin_support::{next_arg_value, parse_finite_f32, parse_view_dimensions},
    project_scene_models_with_view_window, read_world_stream_bytes, BackendSignal, WindowBackend,
    WindowFrame, WindowPresenter,
};
use mdt_world::parse_world_bundle;
use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<(), String> {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(err) if err.starts_with("Usage: ") => {
            println!("{err}");
            return Ok(());
        }
        Err(err) => return Err(err),
    };
    let bytes = read_world_stream_bytes(args.world_stream_hex.as_deref())?;
    let bundle = parse_world_bundle(&bytes)?;
    let session = bundle.loaded_session()?;
    let base_player_position = args
        .player_position
        .unwrap_or_else(|| session.state().player_position());
    let mut presenter = WindowPresenter::new(NullWindowBackend)
        .with_max_view_tiles(args.max_view_tiles.0, args.max_view_tiles.1);
    let mut last_object_count = 0usize;
    let started = Instant::now();

    for frame_id in 0..args.frames {
        let runtime_player_position = if args.animate_player {
            animated_player_position(base_player_position, frame_id)
        } else {
            base_player_position
        };
        let (scene, mut hud) = project_scene_models_with_view_window(
            &session,
            &args.locale,
            Some(runtime_player_position),
            args.max_view_tiles,
        );
        last_object_count = scene.objects.len();
        hud.status_text = format!("{} frame={frame_id}", hud.status_text);
        presenter.present_once(&scene, &hud)?;
    }

    let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    let avg_ms_per_frame = if args.frames == 0 {
        0.0
    } else {
        elapsed_ms / args.frames as f64
    };
    let effective_fps = if elapsed_ms <= f64::EPSILON {
        0.0
    } else {
        (args.frames as f64 * 1000.0) / elapsed_ms
    };

    println!(
        "bench_window: frames={} elapsed_ms={:.3} avg_ms_per_frame={:.3} effective_fps={:.2} map={}x{} objects_per_frame={} animated_player={} max_view_tiles={}:{}",
        args.frames,
        elapsed_ms,
        avg_ms_per_frame,
        effective_fps,
        session.graph().width(),
        session.graph().height(),
        last_object_count,
        if args.animate_player { 1 } else { 0 },
        args.max_view_tiles.0,
        args.max_view_tiles.1,
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NullWindowBackend;

impl WindowBackend for NullWindowBackend {
    fn present(&mut self, _frame: &WindowFrame) -> Result<BackendSignal, String> {
        Ok(BackendSignal::Continue)
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Args {
    locale: String,
    frames: u64,
    max_view_tiles: (usize, usize),
    player_position: Option<(f32, f32)>,
    world_stream_hex: Option<PathBuf>,
    animate_player: bool,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut locale = String::from("en");
    let mut frames = 300u64;
    let mut max_view_tiles = (64usize, 32usize);
    let mut player_x = None;
    let mut player_y = None;
    let mut world_stream_hex = None;
    let mut animate_player = false;
    let mut pending = args.collect::<Vec<_>>().into_iter();

    while let Some(arg) = pending.next() {
        match arg.as_str() {
            "--locale" => {
                locale = next_arg_value(&mut pending, "--locale")?;
            }
            "--frames" => {
                frames = parse_u64(&next_arg_value(&mut pending, "--frames")?)?;
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
            "--animate-player" => {
                animate_player = true;
            }
            "--help" | "-h" => {
                return Err("Usage: mdt-render-ui-window-bench [--locale <locale>] [--frames <n>] [--max-view-tiles <width:height>] [--player-x <f32> --player-y <f32>] [--world-stream-hex <path>] [--animate-player]".to_string());
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

    Ok(Args {
        locale,
        frames,
        max_view_tiles,
        player_position,
        world_stream_hex,
        animate_player,
    })
}

fn animated_player_position(origin: (f32, f32), frame_id: u64) -> (f32, f32) {
    let t = frame_id as f32 / 12.0;
    (origin.0 + t.sin() * 16.0, origin.1 + t.cos() * 12.0)
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value.parse::<u64>().map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Args};
    use std::path::PathBuf;

    fn argv<'a>(parts: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        parts.iter().map(|part| (*part).to_string())
    }

    fn assert_parse_ok(parts: &[&str], expected: Args) {
        let args = parse_args(argv(parts)).expect("expected parse success");
        assert_eq!(args, expected);
    }

    fn assert_parse_err_contains(parts: &[&str], expected: &str) {
        let err = parse_args(argv(parts)).expect_err("expected parse failure");
        assert!(err.contains(expected), "expected `{err}` to contain `{expected}`");
    }

    fn assert_parse_help(parts: &[&str]) {
        let err = parse_args(argv(parts)).expect_err("expected help usage output");
        assert!(err.starts_with("Usage: mdt-render-ui-window-bench"));
    }

    #[test]
    fn parse_args_accepts_bench_options() {
        assert_parse_ok(
            &[
                "--locale",
                "fr",
                "--frames",
                "480",
                "--max-view-tiles",
                "48:24",
                "--player-x",
                "32",
                "--player-y",
                "48",
                "--world-stream-hex",
                "sample.hex",
                "--animate-player",
            ],
            Args {
                locale: "fr".to_string(),
                frames: 480,
                max_view_tiles: (48, 24),
                player_position: Some((32.0, 48.0)),
                world_stream_hex: Some(PathBuf::from("sample.hex")),
                animate_player: true,
            },
        );
    }

    #[test]
    fn parse_args_help_is_not_an_error() {
        assert_parse_help(&["--help"]);
    }

    #[test]
    fn parse_args_rejects_nonfinite_player_coords_and_zero_sizes() {
        assert_parse_err_contains(
            &["--player-x", "NaN", "--player-y", "12"],
            "invalid --player-x: must be finite",
        );

        assert_parse_err_contains(
            &["--max-view-tiles", "0:24", "--player-x", "1", "--player-y", "2"],
            "invalid --max-view-tiles width: must be greater than 0",
        );

        assert_parse_err_contains(
            &["--player-x", "inf", "--player-y", "12"],
            "invalid --player-x: must be finite",
        );
    }
}
