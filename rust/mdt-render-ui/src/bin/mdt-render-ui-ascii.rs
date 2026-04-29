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

    fn argv<'a>(values: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        values.iter().map(|value| (*value).to_string())
    }

    fn assert_parsed_args(values: &[&str], expected: Args) {
        let outcome = parse_args(argv(values)).unwrap();
        match outcome {
            ParseOutcome::Args(args) => assert_eq!(args, expected),
            ParseOutcome::Help(_) => panic!("expected parsed args"),
        }
    }

    fn assert_help_usage_starts_with(values: &[&str], expected_prefix: &str) {
        let outcome = parse_args(argv(values)).unwrap();
        match outcome {
            ParseOutcome::Help(usage) => assert!(usage.starts_with(expected_prefix)),
            ParseOutcome::Args(_) => panic!("expected help"),
        }
    }

    fn assert_parse_err_eq(values: &[&str], expected: &str) {
        assert_eq!(parse_args(argv(values)).unwrap_err(), expected);
    }

    #[test]
    fn parse_args_accepts_optional_hex_path_and_locale() {
        assert_parsed_args(
            &["--locale", "fr", "--world-stream-hex", "sample.hex"],
            Args {
                locale: "fr".to_string(),
                world_stream_hex: Some(PathBuf::from("sample.hex")),
            },
        );
    }

    #[test]
    fn parse_args_help_is_not_an_error() {
        assert_help_usage_starts_with(&["--help"], "Usage: mdt-render-ui-ascii");
    }

    #[test]
    fn parse_args_rejects_duplicate_ascii_flags() {
        assert_parse_err_eq(
            &["--locale", "fr", "--locale", "de"],
            "duplicate argument: --locale",
        );
        assert_parse_err_eq(
            &["--world-stream-hex", "a.hex", "--world-stream-hex", "b.hex"],
            "duplicate argument: --world-stream-hex",
        );
    }
}
