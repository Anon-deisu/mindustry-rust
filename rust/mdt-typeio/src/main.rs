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

    fn argv<'a>(args: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        args.iter().map(|arg| (*arg).to_string())
    }

    fn parse_ok(args: &[&str]) -> PathBuf {
        parse_args(argv(args)).unwrap()
    }

    fn parse_err(args: &[&str]) -> String {
        parse_args(argv(args)).unwrap_err()
    }

    #[test]
    fn rejects_extra_arguments() {
        let err = parse_err(&["out", "extra"]);

        assert_eq!(err, USAGE);
    }

    #[test]
    fn accepts_single_output_dir() {
        let output_dir = parse_ok(&["out"]);

        assert_eq!(output_dir, PathBuf::from("out"));
    }
}
