use std::{
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
};

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_path = env::args().nth(1).ok_or(
        "usage: mdt-remote <manifest-path> [registry-output-path] [high-frequency-output-path]",
    )?;
    let output_path = env::args().nth(2);
    let high_frequency_output_path = env::args().nth(3).or_else(|| {
        output_path
            .as_deref()
            .map(default_high_frequency_output_path)
    });

    let manifest = mdt_remote::read_remote_manifest(&manifest_path)?;
    let generated = mdt_remote::generate_rust_registry(&manifest);
    let generated_high_frequency = mdt_remote::generate_high_frequency_rust_module(&manifest)?;

    if let Some(output_path) = output_path {
        fs::write(&output_path, generated)?;
        if let Some(high_frequency_output_path) = high_frequency_output_path {
            fs::write(high_frequency_output_path, generated_high_frequency)?;
        }
    } else {
        print!("{generated}");
    }

    Ok(())
}

fn default_high_frequency_output_path(output_path: &str) -> String {
    let output_path = PathBuf::from(output_path);
    let output_dir = output_path.parent().unwrap_or_else(|| Path::new("."));
    output_dir
        .join("remote-high-frequency.rs")
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::default_high_frequency_output_path;
    use std::path::Path;

    #[test]
    fn derives_sibling_high_frequency_output_path() {
        let actual = default_high_frequency_output_path("build/mdt-remote/remote-registry.rs");
        assert!(Path::new(&actual).ends_with("build/mdt-remote/remote-high-frequency.rs"));
    }

    #[test]
    fn falls_back_to_current_directory_for_bare_filename() {
        let actual = default_high_frequency_output_path("remote-registry.rs");
        assert!(Path::new(&actual).ends_with("remote-high-frequency.rs"));
    }
}
