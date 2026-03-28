use mdt_render_ui::{
    project_scene_models, read_world_stream_bytes, AsciiScenePresenter, ScenePresenter,
};
use mdt_world::parse_world_bundle;
use std::path::PathBuf;

fn main() -> Result<(), String> {
    match parse_args(std::env::args().skip(1))? {
        ParseOutcome::Help(usage) => {
            println!("{usage}");
            return Ok(());
        }
        ParseOutcome::Args(args) => {
            let bytes = read_world_stream_bytes(args.world_stream_hex.as_deref())?;
            let bundle = parse_world_bundle(&bytes)?;
            let session = bundle.loaded_session()?;
            let (scene, hud) = project_scene_models(&session, &args.locale);
            let mut presenter = AsciiScenePresenter::default();
            presenter.present(&scene, &hud);
            println!("{}", presenter.last_frame());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ParseOutcome {
    Args(Args),
    Help(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Args {
    locale: String,
    world_stream_hex: Option<PathBuf>,
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<ParseOutcome, String> {
    let mut locale = String::from("en");
    let mut world_stream_hex = None;
    let mut locale_set = false;
    let mut world_stream_hex_set = false;
    let mut pending = args.collect::<Vec<_>>().into_iter();

    while let Some(arg) = pending.next() {
        match arg.as_str() {
            "--locale" => {
                if locale_set {
                    return Err("duplicate argument: --locale".to_string());
                }
                locale = pending.next().ok_or("missing value for --locale")?;
                locale_set = true;
            }
            "--world-stream-hex" => {
                if world_stream_hex_set {
                    return Err("duplicate argument: --world-stream-hex".to_string());
                }
                world_stream_hex = Some(PathBuf::from(
                    pending
                        .next()
                        .ok_or("missing value for --world-stream-hex")?,
                ));
                world_stream_hex_set = true;
            }
            "--help" | "-h" => {
                return Ok(ParseOutcome::Help(usage()));
            }
            other => {
                return Err(format!("unknown argument: {other}"));
            }
        }
    }

    Ok(ParseOutcome::Args(Args {
        locale,
        world_stream_hex,
    }))
}

fn usage() -> String {
    "Usage: mdt-render-ui-ascii [--locale <locale>] [--world-stream-hex <path>]".to_string()
}

#[cfg(test)]
mod tests {
    use super::{parse_args, Args, ParseOutcome};
    use std::path::PathBuf;

    #[test]
    fn parse_args_accepts_optional_hex_path_and_locale() {
        let args = match parse_args(
            vec![
                "--locale".to_string(),
                "fr".to_string(),
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
                world_stream_hex: Some(PathBuf::from("sample.hex")),
            }
        );
    }

    #[test]
    fn parse_args_help_is_not_an_error() {
        let outcome = parse_args(vec!["--help".to_string()].into_iter()).unwrap();

        match outcome {
            ParseOutcome::Help(usage) => assert!(usage.starts_with("Usage: mdt-render-ui-ascii")),
            ParseOutcome::Args(_) => panic!("expected help"),
        }
    }

    #[test]
    fn parse_args_rejects_duplicate_ascii_flags() {
        let locale_err = parse_args(
            vec![
                "--locale".to_string(),
                "fr".to_string(),
                "--locale".to_string(),
                "de".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert_eq!(locale_err, "duplicate argument: --locale");

        let hex_err = parse_args(
            vec![
                "--world-stream-hex".to_string(),
                "a.hex".to_string(),
                "--world-stream-hex".to_string(),
                "b.hex".to_string(),
            ]
            .into_iter(),
        )
        .unwrap_err();
        assert_eq!(hex_err, "duplicate argument: --world-stream-hex");
    }
}
