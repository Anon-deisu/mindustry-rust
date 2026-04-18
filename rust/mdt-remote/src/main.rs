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
    let high_frequency_output_path = resolve_auxiliary_output_path(
        high_frequency_output_path,
        output_path.as_deref(),
        default_high_frequency_output_path,
    )?;
    let inbound_dispatch_output_path = resolve_auxiliary_output_path(
        inbound_dispatch_output_path,
        output_path.as_deref(),
        default_inbound_dispatch_output_path,
    )?;

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

fn resolve_auxiliary_output_path(
    explicit_output_path: Option<PathBuf>,
    registry_output_path: Option<&Path>,
    default_output_path: fn(&Path) -> PathBuf,
) -> io::Result<Option<PathBuf>> {
    match explicit_output_path {
        Some(path) => resolve_cli_path(&path).map(Some),
        None => Ok(registry_output_path.map(default_output_path)),
    }
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
    let output_paths = present_output_paths(
        output_path,
        high_frequency_output_path,
        inbound_dispatch_output_path,
    );

    for (index, (label, path)) in output_paths.iter().enumerate() {
        for (other_label, other_path) in output_paths.iter().skip(index + 1) {
            reject_overlapping_output_path_pair(label, path, other_label, other_path)?;
        }
    }

    Ok(())
}

fn present_output_paths<'a>(
    output_path: Option<&'a Path>,
    high_frequency_output_path: Option<&'a Path>,
    inbound_dispatch_output_path: Option<&'a Path>,
) -> Vec<(&'static str, &'a Path)> {
    [
        ("registry", output_path),
        ("high-frequency", high_frequency_output_path),
        ("inbound-dispatch", inbound_dispatch_output_path),
    ]
    .into_iter()
    .filter_map(|(label, path)| path.map(|path| (label, path)))
    .collect()
}

fn reject_overlapping_output_path_pair(
    label: &str,
    path: &Path,
    other_label: &str,
    other_path: &Path,
) -> io::Result<()> {
    let normalized_path = normalize_path_for_overlap(path);
    let normalized_other_path = normalize_path_for_overlap(other_path);

    if paths_overlap(&normalized_path, &normalized_other_path) {
        return Err(overlapping_output_paths_error(
            label,
            path,
            other_label,
            other_path,
        ));
    }

    Ok(())
}

fn overlapping_output_paths_error(
    label: &str,
    path: &Path,
    other_label: &str,
    other_path: &Path,
) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "output paths for {label} and {other_label} must not overlap: '{}' and '{}'",
            path.display(),
            other_path.display()
        ),
    )
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
        let generated_high_frequency =
            generate_optional_output(manifest, high_frequency_output_path, generate_high_frequency)?;
        let generated_inbound_dispatch = generate_optional_output(
            manifest,
            inbound_dispatch_output_path,
            generate_inbound_dispatch,
        )?;

        write_output_file(output_path, &generated)?;
        write_optional_output_file(high_frequency_output_path, generated_high_frequency.as_deref())?;
        write_optional_output_file(
            inbound_dispatch_output_path,
            generated_inbound_dispatch.as_deref(),
        )?;
        Ok(None)
    } else {
        Ok(Some(generate_registry(manifest).map_err(Into::into)?))
    }
}

fn generate_optional_output<T, G, E>(
    manifest: &T,
    output_path: Option<&Path>,
    generate: G,
) -> Result<Option<String>, Box<dyn Error>>
where
    G: FnOnce(&T) -> Result<String, E>,
    E: Into<Box<dyn Error>>,
{
    output_path
        .map(|_| generate(manifest).map_err(Into::into))
        .transpose()
}

fn write_optional_output_file(output_path: Option<&Path>, generated: Option<&str>) -> io::Result<()> {
    if let Some(output_path) = output_path {
        write_output_file(output_path, generated.expect("generated above"))?;
    }
    Ok(())
}

fn write_output_file(path: &Path, contents: &str) -> io::Result<()> {
    ensure_output_parent_dir(path)?;
    fs::write(path, contents).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to write output file '{}': {error}", path.display()),
        )
    })
}

fn ensure_output_parent_dir(path: &Path) -> io::Result<()> {
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

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        default_high_frequency_output_path, default_inbound_dispatch_output_path,
        emit_outputs, normalize_path_for_overlap, parse_args, paths_overlap,
        present_output_paths, reject_overlapping_output_path_pair, reject_overlapping_output_paths,
        resolve_auxiliary_output_path, resolve_cli_path, write_output_file, USAGE,
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
    fn parse_args_preserves_positional_cli_contract_for_registry_high_frequency_and_inbound_dispatch() {
        let parsed = parse_args(vec![
            "manifest.json".to_string(),
            "registry-output.rs".to_string(),
            "high-frequency-output.rs".to_string(),
            "inbound-dispatch-output.rs".to_string(),
        ]
        .into_iter())
        .expect("positional CLI contract should parse");

        assert_eq!(
            parsed,
            (
                "manifest.json".to_string(),
                Some(Path::new("registry-output.rs").to_path_buf()),
                Some(Path::new("high-frequency-output.rs").to_path_buf()),
                Some(Path::new("inbound-dispatch-output.rs").to_path_buf()),
            )
        );
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
    fn parse_args_accepts_manifest_and_registry_only() {
        let parsed = parse_args(vec![
            "manifest.json".to_string(),
            "registry.rs".to_string(),
        ]
        .into_iter())
        .expect("manifest plus registry output should parse");

        assert_eq!(
            parsed,
            (
                "manifest.json".to_string(),
                Some(Path::new("registry.rs").to_path_buf()),
                None,
                None,
            )
        );
    }

    #[test]
    fn parse_args_accepts_manifest_and_high_frequency_only() {
        let parsed = parse_args(vec![
            "manifest.json".to_string(),
            "high-frequency.rs".to_string(),
        ]
        .into_iter())
        .expect("manifest plus high-frequency output should parse");

        assert_eq!(
            parsed,
            (
                "manifest.json".to_string(),
                Some(Path::new("high-frequency.rs").to_path_buf()),
                None,
                None,
            )
        );
    }

    #[test]
    fn parse_args_accepts_manifest_and_inbound_dispatch_only() {
        let parsed = parse_args(vec![
            "manifest.json".to_string(),
            "inbound-dispatch.rs".to_string(),
        ]
        .into_iter())
        .expect("manifest plus inbound-dispatch output should parse");

        assert_eq!(
            parsed,
            (
                "manifest.json".to_string(),
                Some(Path::new("inbound-dispatch.rs").to_path_buf()),
                None,
                None,
            )
        );
    }

    #[test]
    fn parse_args_preserves_positional_cli_contract_for_registry_high_frequency_inbound_outputs() {
        let parsed = parse_args(vec![
            "manifest.json".to_string(),
            "registry.rs".to_string(),
            "high-frequency.rs".to_string(),
            "inbound-dispatch.rs".to_string(),
        ]
        .into_iter())
        .expect("full positional output set should preserve slot order");

        assert_eq!(
            parsed,
            (
                "manifest.json".to_string(),
                Some(PathBuf::from("registry.rs")),
                Some(PathBuf::from("high-frequency.rs")),
                Some(PathBuf::from("inbound-dispatch.rs")),
            )
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
    fn resolve_auxiliary_output_path_prefers_explicit_path_over_registry_default() {
        let original_dir = env::current_dir().expect("current dir");
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-resolve-aux-explicit-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let actual = resolve_auxiliary_output_path(
            Some(PathBuf::from("explicit/high-frequency.rs")),
            Some(Path::new("registry.rs")),
            default_high_frequency_output_path,
        )
        .unwrap();

        assert_eq!(actual, Some(temp_dir.join("explicit/high-frequency.rs")));

        env::set_current_dir(&original_dir).unwrap();
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn resolve_auxiliary_output_path_derives_default_from_registry_path() {
        let actual = resolve_auxiliary_output_path(
            None,
            Some(Path::new("build/mdt-remote/remote-registry.rs")),
            default_inbound_dispatch_output_path,
        )
        .unwrap();

        assert_eq!(
            actual,
            Some(PathBuf::from("build/mdt-remote/remote-inbound-dispatch.rs"))
        );
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
    fn normalize_path_for_overlap_discards_leading_parent_dirs_after_absolute_root() {
        assert_eq!(
            normalize_path_for_overlap(Path::new("/../build/./registry.rs")),
            PathBuf::from("/build/registry.rs")
        );
        assert_eq!(
            normalize_path_for_overlap(Path::new("/../../registry.rs")),
            PathBuf::from("/registry.rs")
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
    fn emit_outputs_with_only_registry_output_writes_registry_and_skips_auxiliary_generators() {
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-emit-registry-only-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let registry_path = temp_dir.join("registry.rs");

        let generated = emit_outputs(
            &(),
            Some(&registry_path),
            None,
            None,
            |_| Ok::<String, Box<dyn std::error::Error>>("registry-body".to_string()),
            |_| -> Result<String, Box<dyn std::error::Error>> {
                panic!("high-frequency generation should not run when no path is provided")
            },
            |_| -> Result<String, Box<dyn std::error::Error>> {
                panic!("inbound-dispatch generation should not run when no path is provided")
            },
        )
        .unwrap();

        assert_eq!(generated, None);
        assert_eq!(fs::read_to_string(&registry_path).unwrap(), "registry-body");

        let _ = fs::remove_dir_all(&temp_dir);
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
    fn emit_outputs_does_not_invoke_auxiliary_generators_when_registry_generation_fails() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mdt-remote-emit-registry-failure-{}-{}",
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
            |_| -> Result<String, Box<dyn std::error::Error>> { Err("boom".into()) },
            |_| -> Result<String, Box<dyn std::error::Error>> {
                panic!("high-frequency generation should not run after a registry failure")
            },
            |_| -> Result<String, Box<dyn std::error::Error>> {
                panic!("inbound-dispatch generation should not run after a registry failure")
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
    fn reject_overlapping_output_paths_allows_similarly_named_sibling_paths() {
        assert!(reject_overlapping_output_paths(
            Some(Path::new("build/mdt-remote/remote-registry.rs")),
            Some(Path::new("build/mdt-remote/remote-registry-data.rs")),
            Some(Path::new("build/mdt-remote/remote-inbound-dispatch.rs")),
        )
        .is_ok());
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
    fn present_output_paths_filters_missing_paths_and_preserves_pairwise_order() {
        let paths = present_output_paths(
            None,
            Some(Path::new("build/mdt-remote/remote-high-frequency.rs")),
            Some(Path::new("build/mdt-output/remote-inbound-dispatch.rs")),
        );

        assert_eq!(
            paths,
            vec![
                (
                    "high-frequency",
                    Path::new("build/mdt-remote/remote-high-frequency.rs")
                ),
                (
                    "inbound-dispatch",
                    Path::new("build/mdt-output/remote-inbound-dispatch.rs")
                ),
            ]
        );
    }

    #[test]
    fn reject_overlapping_output_paths_rejects_high_frequency_and_inbound_dispatch_overlap_without_registry(
    ) {
        let err = reject_overlapping_output_paths(
            None,
            Some(Path::new("build/mdt-remote")),
            Some(Path::new("build/mdt-remote/remote-inbound-dispatch.rs")),
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("output paths for high-frequency and inbound-dispatch must not overlap"));
    }

    #[test]
    fn reject_overlapping_output_paths_rejects_absolute_high_frequency_and_inbound_dispatch_parent_child_overlap_without_registry(
    ) {
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-absolute-non-registry-overlap-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));

        let high_frequency_path = temp_dir.join("build/mdt-remote");
        let inbound_dispatch_path = temp_dir.join("build/mdt-remote/remote-inbound-dispatch.rs");

        let err = reject_overlapping_output_paths(
            None,
            Some(high_frequency_path.as_path()),
            Some(inbound_dispatch_path.as_path()),
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("output paths for high-frequency and inbound-dispatch must not overlap"));

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn reject_overlapping_output_path_pair_canonicalizes_and_rejects_overlap() {
        let err = reject_overlapping_output_path_pair(
            "registry",
            Path::new("build/mdt-remote/registry.rs"),
            "high-frequency",
            Path::new("build/mdt-remote/./registry.rs"),
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("output paths for registry and high-frequency must not overlap"));
    }

    #[test]
    fn reject_overlapping_output_path_pair_rejects_absolute_parent_child_paths_after_resolution() {
        let original_dir = env::current_dir().expect("current dir");
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-absolute-overlap-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let parent_path = resolve_cli_path(Path::new("build/mdt-remote")).unwrap();
        let child_path =
            resolve_cli_path(Path::new("build/mdt-remote/remote-inbound-dispatch.rs")).unwrap();
        let err = reject_overlapping_output_path_pair(
            "high-frequency",
            &parent_path,
            "inbound-dispatch",
            &child_path,
        )
        .unwrap_err();

        assert!(err
            .to_string()
            .contains("output paths for high-frequency and inbound-dispatch must not overlap"));

        env::set_current_dir(&original_dir).unwrap();
        let _ = fs::remove_dir_all(&temp_dir);
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

    #[test]
    fn write_output_file_creates_unicode_parent_directories_and_writes_contents() {
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-write-output-file-unicode-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let output_path = temp_dir.join("嵌套").join("输出.rs");

        write_output_file(&output_path, "generated-内容").unwrap();

        assert_eq!(
            std::fs::read_to_string(&output_path).unwrap(),
            "generated-内容"
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn write_output_file_writes_into_current_directory_without_creating_parent() {
        let original_dir = env::current_dir().expect("current dir");
        let temp_dir = env::temp_dir().join(format!(
            "mdt-remote-write-output-file-bare-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();
        env::set_current_dir(&temp_dir).unwrap();

        let output_path = Path::new("registry.rs");
        write_output_file(output_path, "generated-registry").unwrap();

        assert_eq!(fs::read_to_string(temp_dir.join(output_path)).unwrap(), "generated-registry");

        env::set_current_dir(&original_dir).unwrap();
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
