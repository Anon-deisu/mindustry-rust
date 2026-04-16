use std::{
    env,
    error::Error,
    fs, io,
    path::{Path, PathBuf},
};

const USAGE: &str =
    "usage: mdt-remote <manifest-path> [registry-output-path] [high-frequency-output-path] [inbound-dispatch-output-path]";

fn main() -> Result<(), Box<dyn Error>> {
    let (manifest_path, output_path, high_frequency_output_path, inbound_dispatch_output_path) =
        parse_args(env::args().skip(1))?;
    let output_path = output_path.as_deref().map(resolve_cli_path).transpose()?;
    let high_frequency_output_path = match high_frequency_output_path {
        Some(path) => Some(resolve_cli_path(Path::new(&path))?),
        None => output_path
            .as_deref()
            .map(default_high_frequency_output_path),
    };
    let inbound_dispatch_output_path = match inbound_dispatch_output_path {
        Some(path) => Some(resolve_cli_path(Path::new(&path))?),
        None => output_path
            .as_deref()
            .map(default_inbound_dispatch_output_path),
    };

    reject_overlapping_output_paths(
        output_path.as_deref(),
        high_frequency_output_path.as_deref(),
        inbound_dispatch_output_path.as_deref(),
    )?;

    let manifest = mdt_remote::read_remote_manifest(&manifest_path)?;
    if let Some(generated) = emit_outputs(
        &manifest,
        output_path.as_deref(),
        high_frequency_output_path.as_deref(),
        inbound_dispatch_output_path.as_deref(),
        mdt_remote::generate_rust_registry,
        mdt_remote::generate_high_frequency_rust_module,
        mdt_remote::generate_inbound_dispatch_rust_module,
    )? {
        print!("{generated}");
    }

    Ok(())
}

fn parse_args(
    mut args: impl Iterator<Item = String>,
) -> Result<(String, Option<PathBuf>, Option<PathBuf>, Option<PathBuf>), &'static str> {
    let manifest_path = args.next().ok_or(USAGE)?;
    let output_path = args.next().map(PathBuf::from);
    let high_frequency_output_path = args.next().map(PathBuf::from);
    let inbound_dispatch_output_path = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err(USAGE);
    }

    Ok((
        manifest_path,
        output_path,
        high_frequency_output_path,
        inbound_dispatch_output_path,
    ))
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

fn default_inbound_dispatch_output_path(output_path: &Path) -> PathBuf {
    output_path.with_file_name("remote-inbound-dispatch.rs")
}

fn reject_overlapping_output_paths(
    output_path: Option<&Path>,
    high_frequency_output_path: Option<&Path>,
    inbound_dispatch_output_path: Option<&Path>,
) -> io::Result<()> {
    let output_paths = [
        ("registry", output_path),
        ("high-frequency", high_frequency_output_path),
        ("inbound-dispatch", inbound_dispatch_output_path),
    ];

    for (index, (label, path)) in output_paths.iter().enumerate() {
        let Some(path) = path else {
            continue;
        };
        let normalized_path = normalize_path_for_overlap(path);

        for (other_label, other_path) in output_paths.iter().skip(index + 1) {
            let Some(other_path) = other_path else {
                continue;
            };
            let normalized_other_path = normalize_path_for_overlap(other_path);

            if paths_overlap(&normalized_path, &normalized_other_path) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "output paths for {label} and {other_label} must not overlap: '{}' and '{}'",
                        path.display(),
                        other_path.display()
                    ),
                ));
            }
        }
    }

    Ok(())
}

fn paths_overlap(path: &Path, other_path: &Path) -> bool {
    path.starts_with(other_path) || other_path.starts_with(path)
}

fn normalize_path_for_overlap(path: &Path) -> PathBuf {
    use std::path::Component;

    let mut normalized = PathBuf::new();
    let mut is_absolute = false;

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() && !is_absolute {
                    normalized.push(component.as_os_str());
                }
            }
            Component::RootDir => {
                is_absolute = true;
                normalized.push(component.as_os_str());
            }
            _ => normalized.push(component.as_os_str()),
        }
    }

    normalized
}

fn emit_outputs<T, R, H, I, ER, EH, EI>(
    manifest: &T,
    output_path: Option<&Path>,
    high_frequency_output_path: Option<&Path>,
    inbound_dispatch_output_path: Option<&Path>,
    generate_registry: R,
    generate_high_frequency: H,
    generate_inbound_dispatch: I,
) -> Result<Option<String>, Box<dyn Error>>
where
    R: FnOnce(&T) -> Result<String, ER>,
    H: FnOnce(&T) -> Result<String, EH>,
    I: FnOnce(&T) -> Result<String, EI>,
    ER: Into<Box<dyn Error>>,
    EH: Into<Box<dyn Error>>,
    EI: Into<Box<dyn Error>>,
{
    if let Some(output_path) = output_path {
        let generated = generate_registry(manifest).map_err(Into::into)?;
        let generated_high_frequency = high_frequency_output_path
            .map(|_| generate_high_frequency(manifest).map_err(Into::into))
            .transpose()?;
        let generated_inbound_dispatch = inbound_dispatch_output_path
            .map(|_| generate_inbound_dispatch(manifest).map_err(Into::into))
            .transpose()?;

        write_output_file(output_path, &generated)?;
        if let Some(high_frequency_output_path) = high_frequency_output_path {
            write_output_file(
                high_frequency_output_path,
                generated_high_frequency.as_deref().expect("generated above"),
            )?;
        }
        if let Some(inbound_dispatch_output_path) = inbound_dispatch_output_path {
            write_output_file(
                inbound_dispatch_output_path,
                generated_inbound_dispatch.as_deref().expect("generated above"),
            )?;
        }
        Ok(None)
    } else {
        Ok(Some(generate_registry(manifest).map_err(Into::into)?))
    }
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
    use super::{
        default_high_frequency_output_path, default_inbound_dispatch_output_path,
        emit_outputs, normalize_path_for_overlap, parse_args, paths_overlap,
        reject_overlapping_output_paths, resolve_cli_path, write_output_file, USAGE,
    };
    use std::{
        env, fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

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

    #[test]
    fn derives_sibling_inbound_dispatch_output_path() {
        let actual =
            default_inbound_dispatch_output_path(Path::new("build/mdt-remote/remote-registry.rs"));
        assert!(actual.ends_with("build/mdt-remote/remote-inbound-dispatch.rs"));
    }

    #[test]
    fn inbound_dispatch_output_falls_back_to_current_directory_for_bare_filename() {
        let actual = default_inbound_dispatch_output_path(Path::new("remote-registry.rs"));
        assert!(actual.ends_with("remote-inbound-dispatch.rs"));
    }

    #[test]
    fn rejects_extra_arguments() {
        let err = parse_args(vec![
            "manifest.json".to_string(),
            "registry.rs".to_string(),
            "high-frequency.rs".to_string(),
            "inbound.rs".to_string(),
            "extra.rs".to_string(),
        ]
        .into_iter())
        .unwrap_err();

        assert_eq!(err, USAGE);
    }

    #[test]
    fn parse_args_accepts_required_manifest_and_optional_outputs_then_rejects_missing_manifest() {
        let parsed = parse_args(vec![
            "manifest.json".to_string(),
            "registry.rs".to_string(),
            "high-frequency.rs".to_string(),
            "inbound.rs".to_string(),
        ]
        .into_iter())
        .expect("valid arguments should parse");

        assert_eq!(
            parsed,
            (
                "manifest.json".to_string(),
                Some(Path::new("registry.rs").to_path_buf()),
                Some(Path::new("high-frequency.rs").to_path_buf()),
                Some(Path::new("inbound.rs").to_path_buf()),
            )
        );

        let err = parse_args(Vec::<String>::new().into_iter()).unwrap_err();
        assert_eq!(err, USAGE);
    }

    #[test]
    fn parse_args_accepts_manifest_only_and_leaves_optional_outputs_empty() {
        let parsed = parse_args(vec!["manifest.json".to_string()].into_iter())
            .expect("manifest-only arguments should parse");

        assert_eq!(
            parsed,
            ("manifest.json".to_string(), None, None, None)
        );
    }

    #[test]
    fn resolve_cli_path_keeps_absolute_paths_and_joins_relative_paths() {
        let original_dir = env::current_dir().expect("current dir");
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-resolve-cli-path-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let absolute_path = temp_dir.join("registry.rs");
        let relative_path = Path::new("out/registry.rs");

        assert_eq!(resolve_cli_path(&absolute_path).unwrap(), absolute_path);
        assert_eq!(
            resolve_cli_path(relative_path).unwrap(),
            temp_dir.join(relative_path)
        );

        env::set_current_dir(&original_dir).unwrap();
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn resolve_cli_path_preserves_dot_segments_when_joining_relative_paths() {
        let original_dir = env::current_dir().expect("current dir");
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-resolve-cli-path-dot-segments-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let dot_path = Path::new("out/./registry.rs");
        assert_eq!(resolve_cli_path(dot_path).unwrap(), temp_dir.join(dot_path));

        env::set_current_dir(&original_dir).unwrap();
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn normalize_path_for_overlap_collapses_dot_segments_and_preserves_leading_parents() {
        assert_eq!(
            normalize_path_for_overlap(Path::new("./build/../remote/registry.rs")),
            PathBuf::from("remote/registry.rs")
        );
        assert_eq!(
            normalize_path_for_overlap(Path::new("../remote/./registry.rs")),
            PathBuf::from("../remote/registry.rs")
        );
    }

    #[test]
    fn stdout_registry_generation_does_not_depend_on_auxiliary_generation() {
        let generated = emit_outputs(
            &(),
            None,
            Some(Path::new("high-frequency.rs")),
            Some(Path::new("inbound-dispatch.rs")),
            |_| Ok::<String, Box<dyn std::error::Error>>("registry".to_string()),
            |_| -> Result<String, Box<dyn std::error::Error>> {
                panic!("high-frequency generation should not run in stdout mode")
            },
            |_| -> Result<String, Box<dyn std::error::Error>> {
                panic!("inbound-dispatch generation should not run in stdout mode")
            },
        )
        .unwrap();

        assert_eq!(generated, Some("registry".to_string()));
    }

    #[test]
    fn emit_outputs_does_not_leave_partial_artifacts_on_generation_error() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mdt-remote-emit-outputs-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let registry_path = temp_dir.join("registry.rs");
        let high_frequency_path = temp_dir.join("high-frequency.rs");
        let inbound_dispatch_path = temp_dir.join("inbound-dispatch.rs");

        let err = emit_outputs(
            &(),
            Some(&registry_path),
            Some(&high_frequency_path),
            Some(&inbound_dispatch_path),
            |_| Ok::<String, Box<dyn std::error::Error>>("registry".to_string()),
            |_| -> Result<String, Box<dyn std::error::Error>> { Err("boom".into()) },
            |_| -> Result<String, Box<dyn std::error::Error>> {
                panic!("inbound-dispatch generation should not run after a failure")
            },
        )
        .unwrap_err();

        assert_eq!(err.to_string(), "boom");
        assert!(!registry_path.exists());
        assert!(!high_frequency_path.exists());
        assert!(!inbound_dispatch_path.exists());

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn rejects_overlapping_output_paths() {
        let err = reject_overlapping_output_paths(
            Some(Path::new("build/mdt-remote/remote-registry.rs")),
            Some(Path::new("build/mdt-remote/remote-registry.rs")),
            Some(Path::new("build/mdt-remote/remote-inbound-dispatch.rs")),
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("output paths for registry and high-frequency must not overlap"));
    }

    #[test]
    fn reject_overlapping_output_paths_canonicalizes_relative_segments() {
        let err = reject_overlapping_output_paths(
            Some(Path::new("build/mdt-remote/remote-registry.rs")),
            Some(Path::new("build/mdt-remote/./remote-registry.rs")),
            Some(Path::new("build/mdt-remote/remote-inbound-dispatch.rs")),
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("output paths for registry and high-frequency must not overlap"));
    }

    #[test]
    fn paths_overlap_marks_same_and_ancestor_paths_as_overlapping() {
        let same_path = Path::new("build/mdt-remote/remote-registry.rs");
        let parent_path = Path::new("build/mdt-remote");
        let sibling_path = Path::new("build/mdt-remote/remote-inbound-dispatch.rs");
        let other_root = Path::new("build/mdt-output/remote-registry.rs");

        assert!(paths_overlap(same_path, same_path));
        assert!(paths_overlap(parent_path, same_path));
        assert!(paths_overlap(same_path, parent_path));
        assert!(!paths_overlap(sibling_path, other_root));

        let err = reject_overlapping_output_paths(
            Some(parent_path),
            Some(same_path),
            Some(Path::new("build/mdt-output/remote-inbound-dispatch.rs")),
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("output paths for registry and high-frequency must not overlap"));
    }

    #[test]
    fn reject_overlapping_output_paths_allows_distinct_paths_with_missing_registry() {
        assert!(reject_overlapping_output_paths(
            None,
            Some(Path::new("build/mdt-remote/remote-high-frequency.rs")),
            Some(Path::new("build/mdt-output/remote-inbound-dispatch.rs")),
        )
        .is_ok());
    }

    #[test]
    fn write_output_file_creates_missing_parent_directories_and_writes_contents() {
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-write-output-file-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let output_path = temp_dir.join("nested").join("registry.rs");

        write_output_file(&output_path, "generated-registry").unwrap();

        assert_eq!(
            std::fs::read_to_string(&output_path).unwrap(),
            "generated-registry"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
