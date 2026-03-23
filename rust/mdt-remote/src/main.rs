use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = env::args();
    let _bin = args.next();

    let manifest_path = args.next().ok_or(
        "usage: mdt-remote <manifest-path> [registry-output-path] [high-frequency-output-path]",
    )?;
    let output_path = args.next().map(PathBuf::from);
    let output_path = output_path.as_deref().map(resolve_cli_path).transpose()?;
    let high_frequency_output_path = match args.next() {
        Some(path) => Some(resolve_cli_path(Path::new(&path))?),
        None => output_path
            .as_deref()
            .map(default_high_frequency_output_path),
    };

    let manifest = mdt_remote::read_remote_manifest(&manifest_path)?;
    let generated = mdt_remote::generate_rust_registry(&manifest);
    let generated_high_frequency = mdt_remote::generate_high_frequency_rust_module(&manifest)?;

    if let Some(output_path) = output_path {
        write_output_file(&output_path, &generated)?;
        if let Some(high_frequency_output_path) = high_frequency_output_path {
            write_output_file(&high_frequency_output_path, &generated_high_frequency)?;
        }
    } else {
        print!("{generated}");
    }

    Ok(())
}

fn resolve_cli_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    Ok(env::current_dir()?.join(path))
}

fn default_high_frequency_output_path(output_path: &Path) -> PathBuf {
    output_path.with_file_name("remote-high-frequency.rs")
}

fn write_output_file(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "failed to create output directory '{}' (for '{}'): {error}",
                    parent.display(),
                    path.display()
                ),
            )
        })?;
    }

    fs::write(path, contents).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to write output file '{}': {error}", path.display()),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::default_high_frequency_output_path;
    use std::path::Path;

    #[test]
    fn derives_sibling_high_frequency_output_path() {
        let actual =
            default_high_frequency_output_path(Path::new("build/mdt-remote/remote-registry.rs"));
        assert!(actual.ends_with("build/mdt-remote/remote-high-frequency.rs"));
    }

    #[test]
    fn falls_back_to_current_directory_for_bare_filename() {
        let actual = default_high_frequency_output_path(Path::new("remote-registry.rs"));
        assert!(actual.ends_with("remote-high-frequency.rs"));
    }
}
