use mdt_render_ui::{
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
                locale = pending.next().ok_or("missing value for --locale")?;
            }
            "--frames" => {
                frames = pending
                    .next()
                    .ok_or("missing value for --frames")?
                    .parse::<u64>()
                    .map_err(|err| err.to_string())?;
            }
            "--max-view-tiles" => {
                max_view_tiles =
                    parse_dimensions(&pending.next().ok_or("missing value for --max-view-tiles")?)?;
            }
            "--player-x" => {
                player_x = Some(parse_finite_f32(
                    "--player-x",
                    &pending.next().ok_or("missing value for --player-x")?,
                )?);
            }
            "--player-y" => {
                player_y = Some(parse_finite_f32(
                    "--player-y",
                    &pending.next().ok_or("missing value for --player-y")?,
                )?);
            }
            "--world-stream-hex" => {
                world_stream_hex = Some(PathBuf::from(
                    pending
                        .next()
                        .ok_or("missing value for --world-stream-hex")?,
                ));
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

fn parse_dimensions(value: &str) -> Result<(usize, usize), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err("invalid --max-view-tiles, expected <width:height>".to_string());
    }
    Ok((
        parse_positive_usize("--max-view-tiles width", parts[0])?,
        parse_positive_usize("--max-view-tiles height", parts[1])?,
    ))
}

fn parse_positive_usize(flag: &str, value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|err| format!("invalid {flag}: {err}"))?;
    if parsed == 0 {
        return Err(format!("invalid {flag}: must be greater than 0"));
    }
    Ok(parsed)
}

fn parse_finite_f32(flag: &str, value: &str) -> Result<f32, String> {
    let parsed = value
        .parse::<f32>()
        .map_err(|err| format!("invalid {flag}: {err}"))?;
    if !parsed.is_finite() {
        return Err(format!("invalid {flag}: must be finite"));
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Args};
    use std::path::PathBuf;

    #[test]
    fn parse_args_accepts_bench_options() {
        let args = parse_args(
            vec![
                "--locale".to_string(),
                "fr".to_string(),
                "--frames".to_string(),
                "480".to_string(),
                "--max-view-tiles".to_string(),
                "48:24".to_string(),
                "--player-x".to_string(),
                "32".to_string(),
                "--player-y".to_string(),
                "48".to_string(),
                "--world-stream-hex".to_string(),
                "sample.hex".to_string(),
                "--animate-player".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(
            args,
            Args {
                locale: "fr".to_string(),
                frames: 480,
                max_view_tiles: (48, 24),
                player_position: Some((32.0, 48.0)),
                world_stream_hex: Some(PathBuf::from("sample.hex")),
                animate_player: true,
            }
        );
    }

    #[test]
    fn parse_args_help_is_not_an_error() {
        let err = parse_args(vec!["--help".to_string()].into_iter()).unwrap_err();
        assert!(err.starts_with("Usage: mdt-render-ui-window-bench"));
    }

    #[test]
    fn parse_args_rejects_nonfinite_player_coords_and_zero_sizes() {
        let err = parse_args(
            vec![
                "--player-x".to_string(),
                "NaN".to_string(),
                "--player-y".to_string(),
                "12".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("invalid --player-x: must be finite"));

        let err = parse_args(
            vec![
                "--max-view-tiles".to_string(),
                "0:24".to_string(),
                "--player-x".to_string(),
                "1".to_string(),
                "--player-y".to_string(),
                "2".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("invalid --max-view-tiles width: must be greater than 0"));

        let err = parse_args(
            vec![
                "--player-x".to_string(),
                "inf".to_string(),
                "--player-y".to_string(),
                "12".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("invalid --player-x: must be finite"));
    }

    #[test]
    fn parse_args_rejects_malformed_max_view_tiles_dimensions() {
        let err = parse_args(
            vec![
                "--max-view-tiles".to_string(),
                "48x24".to_string(),
                "--player-x".to_string(),
                "1".to_string(),
                "--player-y".to_string(),
                "2".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert_eq!(err, "invalid --max-view-tiles, expected <width:height>");
    }

    #[test]
    fn parse_args_rejects_missing_frames_value() {
        let err = parse_args(vec!["--frames".to_string()].into_iter()).unwrap_err();
        assert_eq!(err, "missing value for --frames");
    }
}
