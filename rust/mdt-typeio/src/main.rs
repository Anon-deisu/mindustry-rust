use std::{env, fs, path::PathBuf};

const USAGE: &str = "usage: mdt-typeio <output-dir>";

fn main() -> Result<(), String> {
    let output_dir = parse_args(env::args().skip(1))?;

    fs::create_dir_all(&output_dir).map_err(|err| err.to_string())?;

    let text = mdt_typeio::generate_typeio_goldens();
    fs::write(output_dir.join("typeio-goldens.txt"), text).map_err(|err| err.to_string())?;
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<PathBuf, String> {
    let mut args = args;
    let output_dir = args.next().ok_or_else(|| USAGE.to_string())?;
    if args.next().is_some() {
        return Err(USAGE.to_string());
    }

    Ok(PathBuf::from(output_dir))
}

#[cfg(test)]
mod tests {
    use super::{parse_args, USAGE};
    use std::path::PathBuf;

    #[test]
    fn rejects_extra_arguments() {
        let err = parse_args(vec!["out".to_string(), "extra".to_string()].into_iter()).unwrap_err();

        assert_eq!(err, USAGE);
    }

    #[test]
    fn rejects_multiple_extra_arguments() {
        let err = parse_args(
            vec!["out".to_string(), "extra1".to_string(), "extra2".to_string()].into_iter(),
        )
        .unwrap_err();

        assert_eq!(err, USAGE);
    }

    #[test]
    fn accepts_single_output_dir() {
        let output_dir = parse_args(vec!["out".to_string()].into_iter()).unwrap();

        assert_eq!(output_dir, PathBuf::from("out"));
    }

    #[test]
    fn accepts_single_output_dir_with_spaces() {
        let output_dir = parse_args(vec!["out dir".to_string()].into_iter()).unwrap();

        assert_eq!(output_dir, PathBuf::from("out dir"));
    }

    #[test]
    fn accepts_nested_relative_output_dir_path() {
        let output_dir = parse_args(vec!["out/nested".to_string()].into_iter()).unwrap();

        assert_eq!(output_dir, PathBuf::from("out/nested"));
    }

    #[test]
    fn parse_args_preserves_dot_segments_in_output_dir_path() {
        let output_dir = parse_args(vec!["out/./nested".to_string()].into_iter()).unwrap();

        assert_eq!(output_dir, PathBuf::from("out/./nested"));
    }

    #[test]
    fn parse_args_preserves_absolute_output_dir_path() {
        let output_dir = parse_args(vec!["C:/MDT/out".to_string()].into_iter()).unwrap();

        assert_eq!(output_dir, PathBuf::from("C:/MDT/out"));
    }

    #[test]
    fn parse_args_handles_missing_and_single_output_dir() {
        assert_eq!(
            parse_args(Vec::<String>::new().into_iter()).unwrap_err(),
            USAGE
        );
        assert_eq!(
            parse_args(vec!["out".to_string()].into_iter()).unwrap(),
            PathBuf::from("out")
        );
    }
}
