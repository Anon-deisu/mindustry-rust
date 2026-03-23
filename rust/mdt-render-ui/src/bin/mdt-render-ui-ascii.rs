use mdt_render_ui::{project_scene_models, AsciiScenePresenter, ScenePresenter};
use mdt_world::parse_world_bundle;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args = parse_args(std::env::args().skip(1))?;
    let world_stream = match args.world_stream_hex {
        Some(path) => fs::read_to_string(path).map_err(|err| err.to_string())?,
        None => {
            include_str!("../../../../fixtures/world-streams/archipelago-6567-world-stream.hex")
                .to_string()
        }
    };
    let bytes = decode_hex(&world_stream)?;
    let bundle = parse_world_bundle(&bytes)?;
    let session = bundle.loaded_session()?;
    let (scene, hud) = project_scene_models(&session, &args.locale);
    let mut presenter = AsciiScenePresenter::default();
    presenter.present(&scene, &hud);
    println!("{}", presenter.last_frame());
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    locale: String,
    world_stream_hex: Option<PathBuf>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut locale = String::from("en");
    let mut world_stream_hex = None;
    let mut pending = args.collect::<Vec<_>>().into_iter();

    while let Some(arg) = pending.next() {
        match arg.as_str() {
            "--locale" => {
                locale = pending.next().ok_or("missing value for --locale")?;
            }
            "--world-stream-hex" => {
                world_stream_hex = Some(PathBuf::from(
                    pending
                        .next()
                        .ok_or("missing value for --world-stream-hex")?,
                ));
            }
            "--help" | "-h" => {
                return Err(
                    "Usage: mdt-render-ui-ascii [--locale <locale>] [--world-stream-hex <path>]"
                        .to_string(),
                );
            }
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        }
    }

    Ok(Args {
        locale,
        world_stream_hex,
    })
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
    fn parse_args_accepts_optional_hex_path_and_locale() {
        let args = parse_args(
            vec![
                "--locale".to_string(),
                "fr".to_string(),
                "--world-stream-hex".to_string(),
                "sample.hex".to_string(),
            ]
            .into_iter(),
        )
        .unwrap();

        assert_eq!(
            args,
            Args {
                locale: "fr".to_string(),
                world_stream_hex: Some(PathBuf::from("sample.hex")),
            }
        );
    }
}
