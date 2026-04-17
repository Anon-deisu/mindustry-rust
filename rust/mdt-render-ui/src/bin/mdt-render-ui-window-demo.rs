use mdt_render_ui::{
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
                locale = pending.next().ok_or("missing value for --locale")?;
            }
            "--frames" => {
                frames = pending
                    .next()
                    .ok_or("missing value for --frames")?
                    .parse::<u64>()
                    .map_err(|err| err.to_string())?;
            }
            "--fps" => {
                fps = parse_positive_u32(
                    "--fps",
                    &pending.next().ok_or("missing value for --fps")?,
                )?;
            }
            "--tile-pixels" => {
                tile_pixels = pending
                    .next()
                    .ok_or("missing value for --tile-pixels")?
                    .parse::<usize>()
                    .map_err(|err| err.to_string())?;
                if tile_pixels == 0 {
                    return Err("invalid --tile-pixels: must be greater than 0".to_string());
                }
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

fn parse_positive_u32(flag: &str, value: &str) -> Result<u32, String> {
    let parsed = value
        .parse::<u32>()
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

fn usage() -> String {
    "Usage: mdt-render-ui-window-demo [--locale <locale>] [--frames <n>] [--fps <n>] [--tile-pixels <n>] [--max-view-tiles <width:height>] [--player-x <f32> --player-y <f32>] [--world-stream-hex <path>]".to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_args, parse_dimensions, Args, ParseOutcome};
    use std::path::PathBuf;

    #[test]
    fn parse_args_accepts_demo_options() {
        let args = match parse_args(
            vec![
                "--locale".to_string(),
                "fr".to_string(),
                "--frames".to_string(),
                "240".to_string(),
                "--fps".to_string(),
                "20".to_string(),
                "--tile-pixels".to_string(),
                "10".to_string(),
                "--max-view-tiles".to_string(),
                "48:24".to_string(),
                "--player-x".to_string(),
                "32".to_string(),
                "--player-y".to_string(),
                "48".to_string(),
                "--world-stream-hex".to_string(),
                "sample.hex".to_string(),
            ]
            .into_iter(),
        )
        .unwrap()
        {
            ParseOutcome::Args(args) => args,
            ParseOutcome::Help(_) => panic!("expected parsed args"),
        };

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
        let outcome = parse_args(vec!["--help".to_string()].into_iter()).unwrap();

        match outcome {
            ParseOutcome::Help(usage) => {
                assert!(usage.starts_with("Usage: mdt-render-ui-window-demo"))
            }
            ParseOutcome::Args(_) => panic!("expected help"),
        }
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
                "--tile-pixels".to_string(),
                "0".to_string(),
                "--player-x".to_string(),
                "1".to_string(),
                "--player-y".to_string(),
                "2".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("invalid --tile-pixels: must be greater than 0"));

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
    fn parse_args_rejects_zero_fps() {
        let err = parse_args(
            vec![
                "--fps".to_string(),
                "0".to_string(),
                "--player-x".to_string(),
                "1".to_string(),
                "--player-y".to_string(),
                "2".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("invalid --fps: must be greater than 0"));
    }

    #[test]
    fn parse_args_rejects_overflow_fps() {
        let err = parse_args(
            vec![
                "--fps".to_string(),
                "4294967296".to_string(),
                "--player-x".to_string(),
                "1".to_string(),
                "--player-y".to_string(),
                "2".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert!(err.contains("invalid --fps"));
    }

    #[test]
    fn parse_dimensions_rejects_missing_separator() {
        let err = parse_dimensions("48x24").unwrap_err();

        assert_eq!(err, "invalid --max-view-tiles, expected <width:height>");
    }
}
