use mdt_render_ui::{
    project_scene_models, read_world_stream_bytes, AsciiScenePresenter, ScenePresenter,
};
use mdt_world::parse_world_bundle;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    let args = parse_args(std::env::args().skip(1))?;
    let bytes = read_world_stream_bytes(args.world_stream_hex.as_deref())?;
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
