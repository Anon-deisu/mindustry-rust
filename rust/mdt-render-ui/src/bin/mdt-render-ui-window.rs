use mdt_render_ui::{project_scene_models_with_view_window, MinifbWindowBackend, WindowPresenter};
use mdt_world::parse_world_bundle;
use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

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

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut locale = String::from("en");
    let mut world_stream_hex = None;
    let mut duration = Duration::from_millis(4_000);
    let mut frame_time = Duration::from_millis(33);
    let mut tile_pixels = 12usize;
    let mut max_view_tiles = (64usize, 32usize);
    let mut player_x = None;
    let mut player_y = None;
    let mut animate_player = true;
    let mut pending = args.collect::<Vec<_>>().into_iter();

    while let Some(arg) = pending.next() {
        match arg.as_str() {
            "--locale" => locale = pending.next().ok_or("missing value for --locale")?,
            "--world-stream-hex" => {
                world_stream_hex = Some(PathBuf::from(
                    pending
                        .next()
                        .ok_or("missing value for --world-stream-hex")?,
                ));
            }
            "--duration-ms" => {
                duration = Duration::from_millis(parse_u64(
                    "--duration-ms",
                    &pending.next().ok_or("missing value for --duration-ms")?,
                )?);
            }
            "--frame-ms" => {
                frame_time = Duration::from_millis(parse_u64(
                    "--frame-ms",
                    &pending.next().ok_or("missing value for --frame-ms")?,
                )?);
            }
            "--max-view-tiles" => {
                max_view_tiles =
                    parse_dimensions(&pending.next().ok_or("missing value for --max-view-tiles")?)?;
            }
            "--tile-pixels" => {
                tile_pixels = parse_usize(
                    "--tile-pixels",
                    &pending.next().ok_or("missing value for --tile-pixels")?,
                )?
                .max(1);
            }
            "--player-x" => {
                player_x = Some(parse_f32(
                    "--player-x",
                    &pending.next().ok_or("missing value for --player-x")?,
                )?);
            }
            "--player-y" => {
                player_y = Some(parse_f32(
                    "--player-y",
                    &pending.next().ok_or("missing value for --player-y")?,
                )?);
            }
            "--no-animate-player" => animate_player = false,
            "--help" | "-h" => return Err(usage()),
            other => return Err(format!("unknown argument: {other}\n{}", usage())),
        }
    }

    let player_position = match (player_x, player_y) {
        (Some(x), Some(y)) => Some((x, y)),
        (None, None) => None,
        _ => return Err("both --player-x and --player-y are required".to_string()),
    };

    Ok(Args {
        locale,
        world_stream_hex,
        duration,
        frame_time,
        tile_pixels,
        max_view_tiles,
        player_position,
        animate_player,
    })
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

fn parse_dimensions(value: &str) -> Result<(usize, usize), String> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return Err("invalid --max-view-tiles, expected <width:height>".to_string());
    }
    Ok((
        parse_usize("--max-view-tiles width", parts[0])?.max(1),
        parse_usize("--max-view-tiles height", parts[1])?.max(1),
    ))
}

fn parse_u64(flag: &str, value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|err| format!("invalid {flag}: {err}"))
}

fn parse_usize(flag: &str, value: &str) -> Result<usize, String> {
    value
        .parse::<usize>()
        .map_err(|err| format!("invalid {flag}: {err}"))
}

fn parse_f32(flag: &str, value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .map_err(|err| format!("invalid {flag}: {err}"))
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
    use std::time::Duration;

    #[test]
    fn parse_args_accepts_window_configuration() {
        let args = parse_args(
            vec![
                "--locale".to_string(),
                "fr".to_string(),
                "--world-stream-hex".to_string(),
                "sample.hex".to_string(),
                "--duration-ms".to_string(),
                "2500".to_string(),
                "--frame-ms".to_string(),
                "20".to_string(),
                "--tile-pixels".to_string(),
                "10".to_string(),
                "--max-view-tiles".to_string(),
                "48:24".to_string(),
                "--player-x".to_string(),
                "32".to_string(),
                "--player-y".to_string(),
                "48".to_string(),
                "--no-animate-player".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

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
}
