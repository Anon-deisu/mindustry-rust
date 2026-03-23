use mdt_render_ui::{
    project_scene_models_with_view_window, BackendSignal, WindowBackend, WindowFrame,
    WindowPresenter,
};
use mdt_world::parse_world_bundle;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn main() -> Result<(), String> {
    let args = parse_args(std::env::args().skip(1))?;
    let world_stream = match &args.world_stream_hex {
        Some(path) => fs::read_to_string(path).map_err(|err| err.to_string())?,
        None => include_str!("../../../../tests/src/test/resources/world-stream.hex").to_string(),
    };
    let bytes = decode_hex(&world_stream)?;
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
                player_x = Some(
                    pending
                        .next()
                        .ok_or("missing value for --player-x")?
                        .parse::<f32>()
                        .map_err(|err| err.to_string())?,
                );
            }
            "--player-y" => {
                player_y = Some(
                    pending
                        .next()
                        .ok_or("missing value for --player-y")?
                        .parse::<f32>()
                        .map_err(|err| err.to_string())?,
                );
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
        parts[0]
            .parse::<usize>()
            .map_err(|err| err.to_string())?
            .max(1),
        parts[1]
            .parse::<usize>()
            .map_err(|err| err.to_string())?
            .max(1),
    ))
}

fn decode_hex(text: &str) -> Result<Vec<u8>, String> {
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
        .map(|chunk| {
            let pair = std::str::from_utf8(chunk).map_err(|err| err.to_string())?;
            u8::from_str_radix(pair, 16).map_err(|err| err.to_string())
        })
        .collect()
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
}
