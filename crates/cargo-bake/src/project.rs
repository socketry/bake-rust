// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{Result, cargo, options::Options};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{Array, DocumentMut, Item, Table, Value as TomlValue};

#[derive(Deserialize)]
struct Metadata {
    workspace_root: PathBuf,
    metadata: Value,
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    manifest_path: PathBuf,
    metadata: Value,
    default_run: Option<String>,
    targets: Vec<Target>,
    dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
    src_path: PathBuf,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    rename: Option<String>,
    kind: Option<String>,
    optional: bool,
    target: Option<String>,
}

pub(crate) struct Project {
    pub(crate) root: PathBuf,
    pub(crate) task_manifest: PathBuf,
    pub(crate) package: String,
    pub(crate) binary: String,
}

struct Location {
    root: PathBuf,
    workspace_root: PathBuf,
    workspace_manifest: PathBuf,
    task_manifest: PathBuf,
}

fn metadata(manifest: &Path, options: &Options) -> Result<Metadata> {
    metadata_with_command(manifest, options, cargo())
}

fn metadata_with_command(
    manifest: &Path,
    options: &Options,
    mut command: std::process::Command,
) -> Result<Metadata> {
    command
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(manifest)
        .current_dir(
            manifest
                .parent()
                .ok_or("manifest has no parent directory")?,
        );
    options.configure(&mut command);
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed for {} ({}): {}",
            manifest.display(),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    parse_metadata(&output.stdout)
}

fn parse_metadata(output: &[u8]) -> Result<Metadata> {
    Ok(serde_json::from_slice(output)?)
}

fn configured_manifest(metadata: &Value) -> Result<Option<&str>> {
    match metadata.get("bake").and_then(|value| value.get("manifest")) {
        None => Ok(None),
        Some(Value::String(path)) if !path.is_empty() => Ok(Some(path)),
        _ => Err("metadata.bake.manifest must be a nonempty path string".into()),
    }
}

impl Project {
    fn locate(directory: &Path, options: &Options) -> Result<Location> {
        let manifest = match &options.manifest {
            Some(path) => directory.join(path),
            None => directory
                .ancestors()
                .map(|parent| parent.join("Cargo.toml"))
                .find(|candidate| candidate.is_file())
                .ok_or("no Cargo.toml found; run inside a Cargo project or use --manifest-path")?,
        }
        .canonicalize()?;
        let project_metadata = match metadata(&manifest, options) {
            Ok(metadata) => metadata,
            Err(_) if options.regenerate => return locate_from_manifest(&manifest),
            Err(error) => return Err(error),
        };
        locate_with_metadata(manifest, project_metadata)
    }

    pub(crate) fn discover(directory: &Path, options: &Options) -> Result<Self> {
        let location = Self::locate(directory, options)?;
        let task_manifest = location.task_manifest.canonicalize().map_err(|error| {
            format!(
                "cannot open task manifest {}: {error}; create an unpublished bake/ binary crate or run `cargo bake --regenerate` to create it",
                location.task_manifest.display()
            )
        })?;
        let task_metadata = metadata(&task_manifest, options)?;
        let package = task_metadata
            .packages
            .into_iter()
            .find(|package| {
                package
                    .manifest_path
                    .canonicalize()
                    .is_ok_and(|path| path == task_manifest)
            })
            .ok_or("task manifest must identify a package, not a virtual workspace")?;
        let binary = selected_binary(&package)?.name.clone();
        Ok(Self {
            root: location.root,
            task_manifest,
            package: package.name,
            binary,
        })
    }

    pub(crate) fn regenerate(directory: &Path, options: &Options) -> Result<()> {
        let location = Self::locate(directory, options)?;
        Self::regenerate_at_location(&location, options, |path| path.canonicalize())
    }

    fn regenerate_at_location(
        location: &Location,
        options: &Options,
        canonicalize: impl Fn(&Path) -> std::io::Result<PathBuf>,
    ) -> Result<()> {
        let created = !location.task_manifest.is_file();
        if created {
            bootstrap_task_package(location)?;
        }

        let task_manifest = canonicalize(&location.task_manifest)?;
        let task_metadata = metadata(&task_manifest, options)?;
        let package = task_metadata
            .packages
            .into_iter()
            .find(|package| {
                package
                    .manifest_path
                    .canonicalize()
                    .is_ok_and(|path| path == task_manifest)
            })
            .ok_or("task manifest must identify a package, not a virtual workspace")?;
        let binary = selected_binary(&package)?;
        let generated_source = generated_imports(&package);
        // Keep the generated module below the source directory. In particular,
        // a `.rs` file directly under `src/bin/` becomes an extra Cargo binary.
        let generated_path = binary
            .src_path
            .parent()
            .unwrap_or_else(|| unreachable!("Cargo binary source paths have a parent directory"))
            .join("bake_generated_tasks/mod.rs");
        write_if_changed(&generated_path, &generated_source)?;
        add_generated_module(&binary.src_path)?;

        if created {
            println!(
                "Created private task crate at {}",
                location.task_manifest.display()
            );
        }
        println!(
            "Regenerated task dependency links in {}",
            generated_path.display()
        );
        Ok(())
    }
}

fn locate_with_metadata(manifest: PathBuf, project_metadata: Metadata) -> Result<Location> {
    locate_with_metadata_using(manifest, project_metadata, |path| path.canonicalize())
}

fn locate_with_metadata_using(
    manifest: PathBuf,
    project_metadata: Metadata,
    canonicalize: impl Fn(&Path) -> std::io::Result<PathBuf>,
) -> Result<Location> {
    let package = project_metadata.packages.iter().find(|package| {
        package
            .manifest_path
            .canonicalize()
            .is_ok_and(|path| path == manifest)
    });
    let package_manifest = package
        .map(|package| configured_manifest(&package.metadata))
        .transpose()?
        .flatten();
    let workspace_root = canonicalize(&project_metadata.workspace_root)?;
    let (root, relative_manifest) = if let Some(path) = package_manifest {
        (
            manifest
                .parent()
                .unwrap_or_else(|| unreachable!("Cargo manifest paths have a parent directory"))
                .to_path_buf(),
            path,
        )
    } else {
        (
            project_metadata.workspace_root.clone(),
            configured_manifest(&project_metadata.metadata)?.unwrap_or("bake/Cargo.toml"),
        )
    };
    let root = canonicalize(&root)?;
    let task_manifest = root.join(relative_manifest);

    Ok(Location {
        root,
        workspace_manifest: workspace_root.join("Cargo.toml"),
        workspace_root,
        task_manifest,
    })
}

fn table_item<'document>(item: Option<&'document Item>, key: &str) -> Option<&'document Item> {
    item.and_then(Item::as_table)
        .and_then(|table| table.get(key))
}

fn configured_manifest_from_toml(document: &DocumentMut, section: &str) -> Result<Option<String>> {
    let item = table_item(document.as_table().get(section), "metadata")
        .and_then(|item| table_item(Some(item), "bake"))
        .and_then(|item| table_item(Some(item), "manifest"));

    match item {
        None => Ok(None),
        Some(item) => match item.as_value().and_then(TomlValue::as_str) {
            Some(path) if !path.is_empty() => Ok(Some(path.to_owned())),
            _ => Err(
                format!("{section}.metadata.bake.manifest must be a nonempty path string").into(),
            ),
        },
    }
}

#[cfg(test)]
fn workspace_manifest_for(
    manifest: &Path,
    document: &DocumentMut,
) -> Result<(PathBuf, DocumentMut)> {
    workspace_manifest_for_using(manifest, document, |path| path.canonicalize())
}

fn workspace_manifest_for_using(
    manifest: &Path,
    document: &DocumentMut,
    canonicalize: impl Fn(&Path) -> std::io::Result<PathBuf>,
) -> Result<(PathBuf, DocumentMut)> {
    if document.as_table().contains_key("workspace") {
        return Ok((manifest.to_path_buf(), document.clone()));
    }

    if let Some(workspace_path) = table_item(document.as_table().get("package"), "workspace")
        .and_then(|item| item.as_value())
        .and_then(TomlValue::as_str)
    {
        let parent = manifest
            .parent()
            .ok_or("manifest has no parent directory")?;
        let workspace_manifest = parent.join(workspace_path).join("Cargo.toml");
        let workspace_manifest = canonicalize(&workspace_manifest)?;
        let contents = fs::read_to_string(&workspace_manifest)?;
        let document = contents.parse::<DocumentMut>()?;
        return Ok((workspace_manifest, document));
    }

    let parent = manifest
        .parent()
        .ok_or("manifest has no parent directory")?;
    for ancestor in parent.ancestors().skip(1) {
        let candidate = ancestor.join("Cargo.toml");
        let Ok(contents) = fs::read_to_string(&candidate) else {
            continue;
        };
        let Ok(document) = contents.parse::<DocumentMut>() else {
            continue;
        };
        if document.as_table().contains_key("workspace") {
            return Ok((canonicalize(&candidate)?, document));
        }
    }

    Ok((manifest.to_path_buf(), document.clone()))
}

fn locate_from_manifest(manifest: &Path) -> Result<Location> {
    locate_from_manifest_using(manifest, |path| path.canonicalize())
}

fn locate_from_manifest_using(
    manifest: &Path,
    canonicalize: impl Fn(&Path) -> std::io::Result<PathBuf>,
) -> Result<Location> {
    let contents = fs::read_to_string(manifest)?;
    let document = contents.parse::<DocumentMut>()?;
    let (workspace_manifest, workspace_document) =
        workspace_manifest_for_using(manifest, &document, &canonicalize)?;
    let workspace_root = canonicalize(
        workspace_manifest
            .parent()
            .unwrap_or_else(|| unreachable!("Cargo manifest paths have a parent directory")),
    )?;
    let package_root = canonicalize(
        manifest
            .parent()
            .unwrap_or_else(|| unreachable!("Cargo manifest paths have a parent directory")),
    )?;
    let package_manifest = configured_manifest_from_toml(&document, "package")?;
    let root = if package_manifest.is_some() {
        package_root
    } else {
        workspace_root.clone()
    };
    let relative_manifest = package_manifest
        .or(configured_manifest_from_toml(
            &workspace_document,
            "workspace",
        )?)
        .unwrap_or_else(|| "bake/Cargo.toml".to_owned());

    Ok(Location {
        root: root.clone(),
        workspace_manifest,
        workspace_root,
        task_manifest: root.join(relative_manifest),
    })
}

fn selected_binary(package: &Package) -> Result<&Target> {
    let binaries: Vec<_> = package
        .targets
        .iter()
        .filter(|target| target.kind.iter().any(|kind| kind == "bin"))
        .collect();
    if let Some(default_run) = &package.default_run {
        binaries
            .into_iter()
            .find(|target| &target.name == default_run)
            .ok_or_else(|| "task package default-run must name a binary target".into())
    } else if let [binary] = binaries.as_slice() {
        Ok(binary)
    } else {
        Err("task package must have one binary, or select one with package.default-run".into())
    }
}

fn generated_imports(package: &Package) -> String {
    let mut names: Vec<_> = package
        .dependencies
        .iter()
        .filter(|dependency| {
            dependency.kind.is_none() && !dependency.optional && dependency.target.is_none()
        })
        .filter(|dependency| dependency.name != "bake")
        .filter_map(|dependency| {
            let name = dependency
                .rename
                .as_deref()
                .unwrap_or(&dependency.name)
                .replace('-', "_");
            (name != "bake").then_some(name)
        })
        .collect();
    names.sort();
    names.dedup();

    let mut source = String::from("// Generated by `cargo bake --regenerate`; do not edit.\n\n");
    for name in names {
        source.push_str(&format!("use {name} as _;\n"));
    }
    source
}

fn write_if_changed(path: &Path, contents: &str) -> Result<()> {
    if fs::read_to_string(path).is_ok_and(|existing| existing == contents) {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

fn add_generated_module(source_path: &Path) -> Result<()> {
    add_generated_module_using(source_path, remove_legacy_generated_module)
}

fn add_generated_module_using(
    source_path: &Path,
    remove_legacy: impl FnOnce(&Path) -> Result<()>,
) -> Result<()> {
    const PATH_ATTRIBUTE: &str = "#[path = \"bake_generated_tasks/mod.rs\"]";
    const MODULE_ITEM: &str = "mod bake_generated_tasks;";
    const LEGACY_PATH_ATTRIBUTE: &str = "#[path = \"__bake_generated_tasks/mod.rs\"]";
    const LEGACY_MODULE_ITEM: &str = "mod __bake_generated_tasks;";

    let mut source = fs::read_to_string(source_path)?;
    let lines: Vec<_> = source.lines().map(str::trim).collect();
    let has_generated_module = lines
        .windows(2)
        .any(|pair| pair[0] == PATH_ATTRIBUTE && pair[1] == MODULE_ITEM);
    let has_legacy_module = lines
        .windows(2)
        .any(|pair| pair[0] == LEGACY_PATH_ATTRIBUTE && pair[1] == LEGACY_MODULE_ITEM);
    if has_legacy_module {
        let legacy_module = format!("{LEGACY_PATH_ATTRIBUTE}\n{LEGACY_MODULE_ITEM}");
        let replacement = if has_generated_module {
            String::new()
        } else {
            format!("{PATH_ATTRIBUTE}\n{MODULE_ITEM}")
        };
        source = source.replace(&legacy_module, &replacement);
        write_if_changed(source_path, &source)?;
        remove_legacy(source_path)?;
        return Ok(());
    }
    if has_generated_module {
        return Ok(());
    }
    if lines.contains(&MODULE_ITEM) {
        return Err(format!(
            "{} already declares `bake_generated_tasks`; remove or rename that module before regenerating",
            source_path.display()
        )
        .into());
    }

    if !source.ends_with('\n') {
        source.push('\n');
    }
    source.push('\n');
    source.push_str(PATH_ATTRIBUTE);
    source.push('\n');
    source.push_str(MODULE_ITEM);
    source.push('\n');
    write_if_changed(source_path, &source)
}

fn remove_legacy_generated_module(source_path: &Path) -> Result<()> {
    remove_legacy_generated_module_using(source_path, |path| fs::remove_file(path))
}

fn remove_legacy_generated_module_using(
    source_path: &Path,
    remove_file: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<()> {
    const GENERATED_MARKER: &str = "// Generated by `cargo bake --regenerate`; do not edit.";

    let Some(source_directory) = source_path.parent() else {
        return Ok(());
    };
    let legacy_directory = source_directory.join("__bake_generated_tasks");
    let legacy_source = legacy_directory.join("mod.rs");
    if fs::read_to_string(&legacy_source).is_ok_and(|source| source.starts_with(GENERATED_MARKER)) {
        remove_file(&legacy_source)?;
        let _ = fs::remove_dir(legacy_directory);
    }
    Ok(())
}

fn bootstrap_task_package(location: &Location) -> Result<()> {
    bootstrap_task_package_using(location, |path| path.canonicalize())
}

fn bootstrap_task_package_using(
    location: &Location,
    canonicalize: impl Fn(&Path) -> std::io::Result<PathBuf>,
) -> Result<()> {
    let task_directory = location
        .task_manifest
        .parent()
        .ok_or("task manifest has no parent directory")?;
    fs::create_dir_all(task_directory)?;
    let task_directory = canonicalize(task_directory)?;

    let member = task_directory
        .strip_prefix(&location.workspace_root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"));
    let standalone_workspace = match member.as_deref() {
        Some(member) if !member.is_empty() && member != "." => {
            add_workspace_member(&location.workspace_manifest, member)?
        }
        _ => true,
    };

    let project_name = location
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let package_name = package_name(project_name);
    let workspace_section = if standalone_workspace {
        "\n[workspace]\n"
    } else {
        ""
    };
    let manifest = format!(
        "[package]\nname = \"{package_name}-bake\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\nbake = \"{}\"\n{workspace_section}",
        env!("CARGO_PKG_VERSION")
    );
    write_if_changed(&location.task_manifest, &manifest)?;

    let source_path = task_directory.join("src/main.rs");
    if !source_path.exists() {
        write_if_changed(
            &source_path,
            "fn main() -> bake::Result<()> {\n    bake::Registry::discover()?.run()\n}\n",
        )?;
    }

    Ok(())
}

fn package_name(directory_name: &str) -> String {
    let mut name = String::new();
    let mut previous_separator = false;
    for character in directory_name.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator && !name.is_empty() {
            name.push('-');
            previous_separator = true;
        }
    }
    while name.ends_with('-') {
        name.pop();
    }
    if name.is_empty() {
        "project".to_owned()
    } else if name
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_digit())
    {
        format!("project-{name}")
    } else {
        name
    }
}

fn member_pattern_matches(pattern: &str, member: &str) -> bool {
    if pattern == member || matches!(pattern, "**" | "**/*") {
        return true;
    }
    if pattern == "*" {
        return !member.contains('/');
    }
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return member == prefix || member.starts_with(&format!("{prefix}/"));
    }
    if let Some(prefix) = pattern.strip_suffix("/*") {
        return member
            .strip_prefix(&format!("{prefix}/"))
            .is_some_and(|remainder| !remainder.contains('/'));
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return member
            .strip_prefix(prefix)
            .is_some_and(|remainder| !remainder.contains('/'));
    }
    false
}

fn add_workspace_member(manifest_path: &Path, member: &str) -> Result<bool> {
    let original = fs::read_to_string(manifest_path)?;
    let mut document = original.parse::<DocumentMut>()?;
    let workspace = document
        .as_table_mut()
        .entry("workspace")
        .or_insert(Item::Table(Table::new()))
        .as_table_mut()
        .ok_or("workspace must be a Cargo.toml table")?;

    let is_excluded = workspace
        .get("exclude")
        .and_then(Item::as_array)
        .is_some_and(|patterns| {
            patterns
                .iter()
                .filter_map(TomlValue::as_str)
                .any(|pattern| member_pattern_matches(pattern, member))
        });
    if is_excluded {
        return Ok(true);
    }

    let members = workspace
        .entry("members")
        .or_insert(Item::Value(TomlValue::Array(Array::new())))
        .as_array_mut()
        .ok_or("workspace.members must be an array")?;
    if !members
        .iter()
        .filter_map(TomlValue::as_str)
        .any(|pattern| member_pattern_matches(pattern, member))
    {
        members.push(member);
    }

    let updated = document.to_string();
    if updated != original {
        fs::write(manifest_path, updated)?;
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn document(source: &str) -> DocumentMut {
        source.parse().unwrap()
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    fn target(name: &str, source: &Path) -> Target {
        Target {
            name: name.to_owned(),
            kind: vec!["bin".to_owned()],
            src_path: source.to_path_buf(),
        }
    }

    fn package(
        root: &Path,
        targets: Vec<Target>,
        default_run: Option<&str>,
        dependencies: Vec<Dependency>,
    ) -> Package {
        Package {
            name: "task-package".to_owned(),
            manifest_path: root.join("Cargo.toml"),
            metadata: Value::Null,
            default_run: default_run.map(str::to_owned),
            targets,
            dependencies,
        }
    }

    #[test]
    fn reads_optional_package_and_workspace_manifest_overrides() {
        assert_eq!(configured_manifest(&Value::Null).unwrap(), None);
        assert_eq!(
            configured_manifest(&serde_json::json!({"bake": {"manifest": "tasks/Cargo.toml"}}))
                .unwrap(),
            Some("tasks/Cargo.toml")
        );
        for metadata in [
            serde_json::json!({"bake": {"manifest": ""}}),
            serde_json::json!({"bake": {"manifest": false}}),
        ] {
            assert!(configured_manifest(&metadata).is_err());
        }

        assert_eq!(
            configured_manifest_from_toml(&DocumentMut::new(), "workspace").unwrap(),
            None
        );

        let workspace = document("[workspace.metadata.bake]\nmanifest = \"tasks/Cargo.toml\"\n");
        assert_eq!(
            configured_manifest_from_toml(&workspace, "workspace").unwrap(),
            Some("tasks/Cargo.toml".to_owned())
        );

        let package = document("[package.metadata.bake]\nmanifest = \"tools/tasks.toml\"\n");
        assert_eq!(
            configured_manifest_from_toml(&package, "package").unwrap(),
            Some("tools/tasks.toml".to_owned())
        );

        for source in [
            "[workspace.metadata.bake]\nmanifest = \"\"\n",
            "[workspace.metadata.bake]\nmanifest = 42\n",
        ] {
            assert!(configured_manifest_from_toml(&document(source), "workspace").is_err());
        }
    }

    #[test]
    fn locates_from_metadata_and_reports_metadata_path_errors() {
        let directory = tempdir().unwrap();
        let manifest = directory.path().join("package/Cargo.toml");
        write(
            &manifest,
            "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        let manifest = manifest.canonicalize().unwrap();
        let package = |metadata| Package {
            name: "example".to_owned(),
            manifest_path: manifest.clone(),
            metadata,
            default_run: None,
            targets: vec![],
            dependencies: vec![],
        };

        assert!(
            locate_with_metadata(
                manifest.clone(),
                Metadata {
                    workspace_root: directory.path().to_path_buf(),
                    metadata: Value::Null,
                    packages: vec![package(serde_json::json!({"bake": {"manifest": ""}}))],
                },
            )
            .is_err()
        );

        assert!(
            locate_with_metadata_using(
                manifest.clone(),
                Metadata {
                    workspace_root: directory.path().to_path_buf(),
                    metadata: Value::Null,
                    packages: vec![],
                },
                |_| Err(std::io::Error::other("workspace disappeared")),
            )
            .is_err()
        );

        let package_root = manifest.parent().unwrap().to_path_buf();
        assert!(
            locate_with_metadata_using(
                manifest.clone(),
                Metadata {
                    workspace_root: directory.path().to_path_buf(),
                    metadata: Value::Null,
                    packages: vec![package(
                        serde_json::json!({"bake": {"manifest": "tasks/Cargo.toml"}})
                    )],
                },
                |path| {
                    if path == package_root {
                        Err(std::io::Error::other("package root disappeared"))
                    } else {
                        path.canonicalize()
                    }
                },
            )
            .is_err()
        );

        let task_manifest = directory.path().join("tasks/Cargo.toml");
        write(&task_manifest, "not a task package yet\n");
        let location = Location {
            root: directory.path().to_path_buf(),
            workspace_root: directory.path().to_path_buf(),
            workspace_manifest: directory.path().join("Cargo.toml"),
            task_manifest,
        };
        assert!(
            Project::regenerate_at_location(&location, &Options::default(), |_| {
                Err(std::io::Error::other("task manifest disappeared"))
            })
            .is_err()
        );
    }

    #[test]
    fn locates_a_workspace_manifest_from_package_and_ancestor_metadata() {
        let directory = tempdir().unwrap();
        let root = directory.path();
        let workspace_manifest = root.join("Cargo.toml");
        write(&workspace_manifest, "[workspace]\nmembers = []\n");

        let root_document = document("[workspace]\n");
        assert_eq!(
            workspace_manifest_for(&workspace_manifest, &root_document)
                .unwrap()
                .0,
            workspace_manifest
        );

        let package_manifest = root.join("member/Cargo.toml");
        write(
            &package_manifest,
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\nworkspace = \"..\"\n",
        );
        let package_document = document(
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\nworkspace = \"..\"\n",
        );
        assert_eq!(
            workspace_manifest_for(&package_manifest, &package_document)
                .unwrap()
                .0,
            workspace_manifest.canonicalize().unwrap()
        );

        let missing_workspace_document = document(
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\nworkspace = \"../missing\"\n",
        );
        assert!(workspace_manifest_for(&package_manifest, &missing_workspace_document).is_err());

        let unreadable_workspace = tempdir().unwrap();
        fs::create_dir_all(unreadable_workspace.path().join("Cargo.toml")).unwrap();
        let member_manifest = unreadable_workspace.path().join("member/Cargo.toml");
        write(&member_manifest, "[package]\nname = \"member\"\n");
        assert!(
            workspace_manifest_for(
                &member_manifest,
                &document("[package]\nworkspace = \"..\"\n"),
            )
            .is_err()
        );

        let malformed_workspace = tempdir().unwrap();
        write(
            &malformed_workspace.path().join("Cargo.toml"),
            "[workspace\n",
        );
        let member_manifest = malformed_workspace.path().join("member/Cargo.toml");
        write(&member_manifest, "[package]\nname = \"member\"\n");
        assert!(
            workspace_manifest_for(
                &member_manifest,
                &document("[package]\nworkspace = \"..\"\n"),
            )
            .is_err()
        );

        let nested_manifest = root.join("member/nested/Cargo.toml");
        write(
            &nested_manifest,
            "[package]\nname = \"nested\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        let nested_document =
            document("[package]\nname = \"nested\"\nversion = \"0.1.0\"\nedition = \"2024\"\n");
        assert_eq!(
            workspace_manifest_for(&nested_manifest, &nested_document)
                .unwrap()
                .0,
            workspace_manifest.canonicalize().unwrap()
        );
    }

    #[test]
    fn falls_back_when_no_ancestor_is_a_workspace_and_reports_parent_errors() {
        let directory = tempdir().unwrap();
        let manifest = directory.path().join("member/Cargo.toml");
        write(
            &manifest,
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        let package_document =
            document("[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\n");
        write(&directory.path().join("Cargo.toml"), "[workspace\n");
        assert_eq!(
            workspace_manifest_for(&manifest, &package_document)
                .unwrap()
                .0,
            manifest
        );

        let package_workspace = document("[package]\nworkspace = \"..\"\n");
        assert!(workspace_manifest_for(Path::new("/"), &package_workspace).is_err());
        assert!(workspace_manifest_for(Path::new("/"), &DocumentMut::new()).is_err());

        let workspace = directory.path().join("Cargo.toml");
        write(&workspace, "[workspace]\n");
        let nested = directory.path().join("nested/Cargo.toml");
        write(&nested, "[package]\nname = \"nested\"\n");
        let nested_document = document("[package]\nname = \"nested\"\n");
        assert!(
            workspace_manifest_for_using(&nested, &nested_document, |_| {
                Err(std::io::Error::other("workspace disappeared"))
            })
            .is_err()
        );

        let calls = std::cell::Cell::new(0);
        assert!(
            locate_from_manifest_using(&workspace, |_| {
                let call = calls.get();
                calls.set(call + 1);
                if call == 1 {
                    Err(std::io::Error::other("package root disappeared"))
                } else {
                    Ok(directory.path().to_path_buf())
                }
            })
            .is_err()
        );
        assert!(
            locate_from_manifest_using(&workspace, |_| {
                Err(std::io::Error::other("workspace root disappeared"))
            })
            .is_err()
        );
    }

    #[test]
    fn reports_metadata_process_and_response_errors() {
        let directory = tempdir().unwrap();
        let options = Options::default();
        assert!(metadata_with_command(Path::new("/"), &options, cargo()).is_err());

        let missing_cargo = directory.path().join("missing-cargo");
        assert!(
            metadata_with_command(
                &directory.path().join("Cargo.toml"),
                &options,
                std::process::Command::new(missing_cargo),
            )
            .is_err()
        );
        assert!(parse_metadata(b"not json").is_err());
    }

    #[test]
    fn locates_package_and_workspace_task_manifest_overrides() {
        let directory = tempdir().unwrap();
        let root = directory.path();
        let workspace_manifest = root.join("Cargo.toml");
        write(
            &workspace_manifest,
            "[workspace]\nmembers = [\"application\"]\n[workspace.metadata.bake]\nmanifest = \"shared/tasks/Cargo.toml\"\n",
        );
        let package_manifest = root.join("application/Cargo.toml");
        write(
            &package_manifest,
            "[package]\nname = \"application\"\nversion = \"0.1.0\"\nedition = \"2024\"\nworkspace = \"..\"\n[package.metadata.bake]\nmanifest = \"tools/Cargo.toml\"\n",
        );

        let location = locate_from_manifest(&package_manifest).unwrap();
        let package_root = package_manifest.parent().unwrap().canonicalize().unwrap();
        assert_eq!(location.root, package_root);
        assert_eq!(location.workspace_root, root.canonicalize().unwrap());
        assert_eq!(
            location.task_manifest,
            package_root.join("tools/Cargo.toml")
        );

        let workspace_location = locate_from_manifest(&workspace_manifest).unwrap();
        assert_eq!(workspace_location.root, root.canonicalize().unwrap());
        assert_eq!(
            workspace_location.task_manifest,
            root.canonicalize().unwrap().join("shared/tasks/Cargo.toml")
        );

        write(
            &directory.path().join("plain/Cargo.toml"),
            "[workspace]\nmembers = [\"plain\"]\n",
        );
        let package_without_override = directory.path().join("plain/member/Cargo.toml");
        write(
            &package_without_override,
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2024\"\nworkspace = \"..\"\n[package.metadata.bake]\nmanifest = \"tasks/Cargo.toml\"\n",
        );
        let package_location = locate_from_manifest(&package_without_override).unwrap();
        assert_eq!(
            package_location.task_manifest,
            package_without_override
                .parent()
                .unwrap()
                .canonicalize()
                .unwrap()
                .join("tasks/Cargo.toml")
        );

        let isolated = tempdir().unwrap();
        let no_override_manifest = isolated.path().join("Cargo.toml");
        write(
            &no_override_manifest,
            "[package]\nname = \"standalone\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        let default_location = locate_from_manifest(&no_override_manifest).unwrap();
        assert_eq!(
            default_location.task_manifest,
            no_override_manifest
                .parent()
                .unwrap()
                .canonicalize()
                .unwrap()
                .join("bake/Cargo.toml")
        );
    }

    #[test]
    fn reports_invalid_workspace_task_manifest_configuration() {
        let directory = tempdir().unwrap();
        let manifest = directory.path().join("Cargo.toml");
        write(
            &manifest,
            "[workspace]\nmembers = []\n[workspace.metadata.bake]\nmanifest = \"\"\n",
        );

        assert!(locate_from_manifest(&manifest).is_err());

        assert!(locate_from_manifest(&manifest.with_file_name("missing.toml")).is_err());

        write(&manifest, "[workspace\n");
        assert!(locate_from_manifest(&manifest).is_err());
    }

    #[test]
    fn reports_project_manifest_and_task_package_errors() {
        let directory = tempdir().unwrap();
        let mut options = Options {
            manifest: Some(PathBuf::from("missing.toml")),
            ..Options::default()
        };
        assert!(Project::locate(directory.path(), &options).is_err());

        write(
            &directory.path().join("Cargo.toml"),
            "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[package.metadata.bake]\nmanifest = \"\"\n",
        );
        options.manifest = None;
        assert!(Project::locate(directory.path(), &options).is_err());

        let workspace = tempdir().unwrap();
        write(
            &workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n[workspace.metadata.bake]\nmanifest = \"Cargo.toml\"\n",
        );
        assert!(Project::discover(workspace.path(), &Options::default()).is_err());

        let library_workspace = tempdir().unwrap();
        write(
            &library_workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"tasks\"]\n[workspace.metadata.bake]\nmanifest = \"tasks/Cargo.toml\"\n",
        );
        write(
            &library_workspace.path().join("tasks/Cargo.toml"),
            "[package]\nname = \"task-library\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write(
            &library_workspace.path().join("tasks/src/lib.rs"),
            "// A task package without a binary.\n",
        );
        assert!(Project::discover(library_workspace.path(), &Options::default()).is_err());
        assert!(Project::regenerate(library_workspace.path(), &Options::default()).is_err());

        let invalid_task_workspace = tempdir().unwrap();
        write(
            &invalid_task_workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n[workspace.metadata.bake]\nmanifest = \"tasks/Cargo.toml\"\n",
        );
        write(
            &invalid_task_workspace.path().join("tasks/Cargo.toml"),
            "[package\n",
        );
        assert!(Project::discover(invalid_task_workspace.path(), &Options::default()).is_err());

        let missing_package_workspace = tempdir().unwrap();
        write(
            &missing_package_workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n[workspace.metadata.bake]\nmanifest = \"Cargo.toml\"\n",
        );
        assert!(
            Project::regenerate(missing_package_workspace.path(), &Options::default()).is_err()
        );

        let blocked_task_workspace = tempdir().unwrap();
        write(
            &blocked_task_workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\n[workspace.metadata.bake]\nmanifest = \"tasks/Cargo.toml\"\n",
        );
        fs::write(
            blocked_task_workspace.path().join("tasks"),
            "not a directory",
        )
        .unwrap();
        assert!(Project::regenerate(blocked_task_workspace.path(), &Options::default()).is_err());

        let generated_module_workspace = tempdir().unwrap();
        write(
            &generated_module_workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"bake\"]\n[workspace.metadata.bake]\nmanifest = \"bake/Cargo.toml\"\n",
        );
        write(
            &generated_module_workspace.path().join("bake/Cargo.toml"),
            "[package]\nname = \"example-bake\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write(
            &generated_module_workspace.path().join("bake/src/main.rs"),
            "fn main() {}\n",
        );
        fs::write(
            generated_module_workspace
                .path()
                .join("bake/src/bake_generated_tasks"),
            "not a directory",
        )
        .unwrap();
        assert!(
            Project::regenerate(generated_module_workspace.path(), &Options::default()).is_err()
        );

        let manual_module_workspace = tempdir().unwrap();
        write(
            &manual_module_workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"bake\"]\n[workspace.metadata.bake]\nmanifest = \"bake/Cargo.toml\"\n",
        );
        write(
            &manual_module_workspace.path().join("bake/Cargo.toml"),
            "[package]\nname = \"example-bake\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        write(
            &manual_module_workspace.path().join("bake/src/main.rs"),
            "mod bake_generated_tasks;\n",
        );
        assert!(Project::regenerate(manual_module_workspace.path(), &Options::default()).is_err());
    }

    #[test]
    fn falls_back_to_manifest_parsing_when_cargo_metadata_fails() {
        let directory = tempdir().unwrap();
        write(
            &directory.path().join("Cargo.toml"),
            "[package]\nname = \"invalid/name\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        );
        let mut options = Options {
            offline: true,
            ..Options::default()
        };
        assert!(Project::locate(directory.path(), &options).is_err());

        options.regenerate = true;
        assert!(Project::locate(directory.path(), &options).is_ok());
    }

    #[test]
    fn discovers_and_regenerates_a_task_package() {
        let directory = tempdir().unwrap();
        let root = directory.path();
        write(
            &root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"bake\"]\nresolver = \"3\"\n",
        );
        write(
            &root.join("bake/Cargo.toml"),
            "[package]\nname = \"example-bake\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n",
        );
        write(&root.join("bake/src/main.rs"), "fn main() {}\n");

        let options = Options::default();
        let package_location = Project::locate(&root.join("bake"), &options).unwrap();
        assert_eq!(package_location.root, root.canonicalize().unwrap());

        let project = Project::discover(root, &options).unwrap();
        assert_eq!(project.package, "example-bake");
        assert_eq!(project.binary, "example-bake");
        assert_eq!(
            project.task_manifest,
            root.join("bake/Cargo.toml").canonicalize().unwrap()
        );

        Project::regenerate(root, &options).unwrap();
        let source = fs::read_to_string(root.join("bake/src/main.rs")).unwrap();
        assert!(source.contains("mod bake_generated_tasks;"));
        assert!(root.join("bake/src/bake_generated_tasks/mod.rs").is_file());

        let empty_workspace = tempdir().unwrap();
        write(
            &empty_workspace.path().join("Cargo.toml"),
            "[workspace]\nmembers = []\nresolver = \"3\"\n",
        );
        let error = Project::discover(empty_workspace.path(), &options)
            .err()
            .unwrap();
        assert!(error.to_string().contains("cannot open task manifest"));
    }

    #[test]
    fn regenerates_a_missing_task_package_and_reports_metadata_failures() {
        let directory = tempdir().unwrap();
        let root = directory.path();
        let workspace_manifest = root.join("Cargo.toml");
        write(&workspace_manifest, "[workspace]\nmembers = []\n");
        let location = Location {
            root: root.canonicalize().unwrap(),
            workspace_root: root.canonicalize().unwrap(),
            workspace_manifest,
            task_manifest: root.join("bake/Cargo.toml"),
        };

        Project::regenerate_at_location(&location, &Options::default(), |path| path.canonicalize())
            .unwrap();
        assert!(location.task_manifest.is_file());

        let invalid_directory = tempdir().unwrap();
        let invalid_root = invalid_directory.path();
        let invalid_workspace_manifest = invalid_root.join("Cargo.toml");
        write(
            &invalid_workspace_manifest,
            "[workspace]\nmembers = [\"bake\"]\n",
        );
        let invalid_manifest = invalid_root.join("bake/Cargo.toml");
        write(&invalid_manifest, "[package\n");
        let invalid_location = Location {
            root: invalid_root.canonicalize().unwrap(),
            workspace_root: invalid_root.canonicalize().unwrap(),
            workspace_manifest: invalid_workspace_manifest,
            task_manifest: invalid_manifest,
        };

        assert!(
            Project::regenerate_at_location(&invalid_location, &Options::default(), |path| path
                .canonicalize(),)
            .is_err()
        );
    }

    #[test]
    fn selects_the_requested_binary_or_requires_one_unambiguous_binary() {
        let directory = tempdir().unwrap();
        let single = package(
            directory.path(),
            vec![target("first", &directory.path().join("src/main.rs"))],
            None,
            vec![],
        );
        assert_eq!(selected_binary(&single).unwrap().name, "first");

        let selected = package(
            directory.path(),
            vec![
                target("first", &directory.path().join("src/main.rs")),
                target("second", &directory.path().join("src/bin/second.rs")),
            ],
            Some("second"),
            vec![],
        );
        assert_eq!(selected_binary(&selected).unwrap().name, "second");

        let missing = package(
            directory.path(),
            vec![
                target("first", &directory.path().join("src/main.rs")),
                target("second", &directory.path().join("src/bin/second.rs")),
            ],
            Some("missing"),
            vec![],
        );
        assert!(selected_binary(&missing).is_err());

        let none = package(directory.path(), vec![], None, vec![]);
        let ambiguous = package(
            directory.path(),
            vec![
                target("first", &directory.path().join("src/main.rs")),
                target("second", &directory.path().join("src/bin/second.rs")),
            ],
            None,
            vec![],
        );
        assert!(selected_binary(&none).is_err());
        assert!(selected_binary(&ambiguous).is_err());
    }

    #[test]
    fn generates_sorted_imports_for_supported_dependencies_only() {
        let directory = tempdir().unwrap();
        let dependency = |name: &str,
                          rename: Option<&str>,
                          kind: Option<&str>,
                          optional: bool,
                          target: Option<&str>| Dependency {
            name: name.to_owned(),
            rename: rename.map(str::to_owned),
            kind: kind.map(str::to_owned),
            optional,
            target: target.map(str::to_owned),
        };
        let package = package(
            directory.path(),
            vec![],
            None,
            vec![
                dependency("z-library", None, None, false, None),
                dependency("my-library", None, None, false, None),
                dependency("renamed-package", Some("public-name"), None, false, None),
                dependency("z-library", None, None, false, None),
                dependency("development-only", None, Some("dev"), false, None),
                dependency("optional", None, None, true, None),
                dependency("platform-only", None, None, false, Some("cfg(unix)")),
                dependency("bake", None, None, false, None),
                dependency("renamed-bake", Some("bake"), None, false, None),
            ],
        );

        assert_eq!(
            generated_imports(&package),
            "// Generated by `cargo bake --regenerate`; do not edit.\n\nuse my_library as _;\nuse public_name as _;\nuse z_library as _;\n"
        );
    }

    #[test]
    fn writes_only_when_generated_contents_change() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("nested/generated.rs");

        write_if_changed(&path, "generated\n").unwrap();
        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        write_if_changed(&path, "generated\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "generated\n");
        assert_eq!(fs::metadata(path).unwrap().modified().unwrap(), modified);
    }

    #[test]
    fn reports_filesystem_write_failures() {
        let directory = tempdir().unwrap();
        let blocker = directory.path().join("blocker");
        fs::write(&blocker, "file").unwrap();

        assert!(write_if_changed(&blocker.join("nested/generated.rs"), "generated").is_err());

        let directory_path = directory.path().join("directory");
        fs::create_dir(&directory_path).unwrap();
        assert!(write_if_changed(&directory_path, "generated").is_err());

        assert!(write_if_changed(Path::new("/"), "generated").is_err());
    }

    #[test]
    fn adds_migrates_and_validates_the_generated_module_declaration() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("src/main.rs");
        assert!(add_generated_module(&source).is_err());
        write(&source, "fn main() {}\n");
        add_generated_module(&source).unwrap();
        let generated = fs::read_to_string(&source).unwrap();
        assert!(generated.contains("#[path = \"bake_generated_tasks/mod.rs\"]"));
        add_generated_module(&source).unwrap();
        assert_eq!(fs::read_to_string(&source).unwrap(), generated);

        let manual_module = directory.path().join("manual.rs");
        write(&manual_module, "mod bake_generated_tasks;\n");
        assert!(add_generated_module(&manual_module).is_err());

        let source_without_final_newline = directory.path().join("without-newline.rs");
        write(&source_without_final_newline, "fn main() {} ");
        add_generated_module(&source_without_final_newline).unwrap();
        assert!(
            fs::read_to_string(&source_without_final_newline)
                .unwrap()
                .contains("fn main() {} \n\n")
        );

        let legacy_source = directory.path().join("legacy/main.rs");
        write(
            &legacy_source,
            "fn main() {}\n#[path = \"__bake_generated_tasks/mod.rs\"]\nmod __bake_generated_tasks;\n",
        );
        let legacy_module = legacy_source
            .parent()
            .unwrap()
            .join("__bake_generated_tasks/mod.rs");
        write(
            &legacy_module,
            "// Generated by `cargo bake --regenerate`; do not edit.\nuse dependency as _;\n",
        );
        add_generated_module(&legacy_source).unwrap();
        assert!(
            fs::read_to_string(&legacy_source)
                .unwrap()
                .contains("mod bake_generated_tasks;")
        );
        assert!(!legacy_module.exists());

        let failed_legacy_source = directory.path().join("failed-legacy/main.rs");
        write(
            &failed_legacy_source,
            "#[path = \"__bake_generated_tasks/mod.rs\"]\nmod __bake_generated_tasks;\n",
        );
        assert!(
            add_generated_module_using(&failed_legacy_source, |_| {
                Err("could not remove generated module".into())
            })
            .is_err()
        );

        let readonly_legacy_source = directory.path().join("readonly-legacy/main.rs");
        write(
            &readonly_legacy_source,
            "#[path = \"__bake_generated_tasks/mod.rs\"]\nmod __bake_generated_tasks;\n",
        );
        let mut permissions = fs::metadata(&readonly_legacy_source).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&readonly_legacy_source, permissions).unwrap();
        assert!(add_generated_module(&readonly_legacy_source).is_err());

        let both_source = directory.path().join("both/main.rs");
        write(
            &both_source,
            "#[path = \"bake_generated_tasks/mod.rs\"]\nmod bake_generated_tasks;\n#[path = \"__bake_generated_tasks/mod.rs\"]\nmod __bake_generated_tasks;\n",
        );
        add_generated_module(&both_source).unwrap();
        assert!(
            !fs::read_to_string(&both_source)
                .unwrap()
                .contains("__bake_generated_tasks")
        );
    }

    #[test]
    fn removes_only_marked_legacy_generated_modules() {
        let directory = tempdir().unwrap();
        let source = directory.path().join("src/main.rs");
        write(&source, "fn main() {}\n");
        let legacy_directory = source.parent().unwrap().join("__bake_generated_tasks");
        write(
            &legacy_directory.join("mod.rs"),
            "// Generated by `cargo bake --regenerate`; do not edit.\n",
        );
        remove_legacy_generated_module(&source).unwrap();
        assert!(!legacy_directory.exists());

        write(&legacy_directory.join("mod.rs"), "project-owned source\n");
        remove_legacy_generated_module(&source).unwrap();
        assert!(legacy_directory.join("mod.rs").is_file());

        write(
            &legacy_directory.join("mod.rs"),
            "// Generated by `cargo bake --regenerate`; do not edit.\n",
        );
        write(&legacy_directory.join("keep.rs"), "fn keep() {}\n");
        remove_legacy_generated_module(&source).unwrap();
        assert!(legacy_directory.join("keep.rs").is_file());

        write(
            &legacy_directory.join("mod.rs"),
            "// Generated by `cargo bake --regenerate`; do not edit.\n",
        );
        assert!(
            remove_legacy_generated_module_using(&source, |_| {
                Err(std::io::Error::other("file is locked"))
            })
            .is_err()
        );

        remove_legacy_generated_module(Path::new("/")).unwrap();
    }

    #[test]
    fn bootstraps_workspace_member_and_standalone_task_packages() {
        let directory = tempdir().unwrap();
        let root = directory.path().join("project-name");
        let workspace_manifest = root.join("Cargo.toml");
        write(&workspace_manifest, "[workspace]\nmembers = []\n");
        let location = Location {
            root: root.canonicalize().unwrap(),
            workspace_root: root.canonicalize().unwrap(),
            workspace_manifest: workspace_manifest.clone(),
            task_manifest: root.join("bake/Cargo.toml"),
        };

        bootstrap_task_package(&location).unwrap();
        let manifest = fs::read_to_string(&location.task_manifest).unwrap();
        assert!(manifest.contains("name = \"project-name-bake\""));
        assert!(!manifest.contains("[workspace]"));
        assert!(
            fs::read_to_string(&workspace_manifest)
                .unwrap()
                .contains("\"bake\"")
        );
        let source = location.task_manifest.parent().unwrap().join("src/main.rs");
        assert!(
            fs::read_to_string(&source)
                .unwrap()
                .contains("Registry::discover")
        );
        fs::write(&source, "// keep project task source\n").unwrap();
        bootstrap_task_package(&location).unwrap();
        assert_eq!(
            fs::read_to_string(&source).unwrap(),
            "// keep project task source\n"
        );

        let standalone_root = directory.path().join("standalone");
        let standalone_workspace = directory.path().join("workspace");
        write(
            &standalone_workspace.join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        );
        let standalone = Location {
            root: standalone_root.clone(),
            workspace_root: standalone_workspace.clone(),
            workspace_manifest: standalone_workspace.join("Cargo.toml"),
            task_manifest: standalone_root.join("bake/Cargo.toml"),
        };
        bootstrap_task_package(&standalone).unwrap();
        assert!(
            fs::read_to_string(&standalone.task_manifest)
                .unwrap()
                .contains("[workspace]")
        );
    }

    #[test]
    fn reports_task_source_bootstrap_write_failures() {
        let directory = tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        write(&root.join("Cargo.toml"), "[workspace]\nmembers = []\n");
        let task_directory = root.join("tasks");
        fs::create_dir_all(&task_directory).unwrap();
        fs::write(task_directory.join("src"), "not a directory").unwrap();

        let location = Location {
            root: root.clone(),
            workspace_root: root.clone(),
            workspace_manifest: root.join("Cargo.toml"),
            task_manifest: task_directory.join("Cargo.toml"),
        };

        assert!(bootstrap_task_package(&location).is_err());

        let no_manifest_parent = Location {
            root: root.clone(),
            workspace_root: root.clone(),
            workspace_manifest: root.join("Cargo.toml"),
            task_manifest: PathBuf::new(),
        };
        assert!(bootstrap_task_package(&no_manifest_parent).is_err());

        let canonicalize_failure = Location {
            root: root.clone(),
            workspace_root: root.clone(),
            workspace_manifest: root.join("Cargo.toml"),
            task_manifest: root.join("canonicalize-failure/Cargo.toml"),
        };
        assert!(
            bootstrap_task_package_using(&canonicalize_failure, |_| {
                Err(std::io::Error::other("task directory disappeared"))
            })
            .is_err()
        );

        let missing_workspace_location = Location {
            root: root.clone(),
            workspace_root: root.clone(),
            workspace_manifest: root.join("missing-workspace.toml"),
            task_manifest: root.join("other-tasks/Cargo.toml"),
        };
        assert!(bootstrap_task_package(&missing_workspace_location).is_err());

        let manifest_directory = root.join("manifest-directory");
        write(
            &manifest_directory.join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        );
        let task_manifest = manifest_directory.join("bake/Cargo.toml");
        fs::create_dir_all(&task_manifest).unwrap();
        let manifest_location = Location {
            root: manifest_directory.canonicalize().unwrap(),
            workspace_root: manifest_directory.canonicalize().unwrap(),
            workspace_manifest: manifest_directory.join("Cargo.toml"),
            task_manifest,
        };
        assert!(bootstrap_task_package(&manifest_location).is_err());
    }

    #[test]
    fn normalizes_generated_package_names() {
        assert_eq!(package_name("Socketry-Rust"), "socketry-rust");
        assert_eq!(package_name("many---separators"), "many-separators");
        assert_eq!(
            package_name("trailing-separators---"),
            "trailing-separators"
        );
        assert_eq!(package_name("123-project"), "project-123-project");
        assert_eq!(package_name("---"), "project");
    }

    #[test]
    fn matches_workspace_member_globs() {
        assert!(member_pattern_matches("crates/task", "crates/task"));
        assert!(member_pattern_matches("**", "nested/crate"));
        assert!(member_pattern_matches("**/*", "nested/crate"));
        assert!(member_pattern_matches("*", "crate"));
        assert!(!member_pattern_matches("*", "nested/crate"));
        assert!(member_pattern_matches("crates/**", "crates/nested/task"));
        assert!(member_pattern_matches("crates/*", "crates/task"));
        assert!(!member_pattern_matches("crates/*", "crates/nested/task"));
        assert!(member_pattern_matches("crates/task*", "crates/task-runner"));
        assert!(!member_pattern_matches(
            "crates/task*",
            "crates/task/nested"
        ));
        assert!(!member_pattern_matches("other/*", "crates/task"));
        assert!(!member_pattern_matches("plain", "different"));
    }

    #[test]
    fn adds_workspace_members_without_overriding_patterns_or_exclusions() {
        let directory = tempdir().unwrap();
        let manifest = directory.path().join("Cargo.toml");

        write(&manifest, "[package]\nname = \"root\"\n");
        assert!(!add_workspace_member(&manifest, "bake").unwrap());
        assert!(
            fs::read_to_string(&manifest)
                .unwrap()
                .contains("members = [\"bake\"]")
        );

        write(
            &manifest,
            "[workspace]\nmembers = [\"tools/*\", 42]\nexclude = \"ignored\"\n",
        );
        let original = fs::read_to_string(&manifest).unwrap();
        assert!(!add_workspace_member(&manifest, "tools/bake").unwrap());
        assert_eq!(fs::read_to_string(&manifest).unwrap(), original);

        write(&manifest, "[workspace]\nexclude = [\"tools/**\"]\n");
        assert!(add_workspace_member(&manifest, "tools/bake").unwrap());

        write(&manifest, "workspace = \"not a table\"\n");
        assert!(add_workspace_member(&manifest, "bake").is_err());

        write(&manifest, "[workspace]\nmembers = \"not an array\"\n");
        assert!(add_workspace_member(&manifest, "bake").is_err());

        write(&manifest, "[workspace\n");
        assert!(add_workspace_member(&manifest, "bake").is_err());
        assert!(add_workspace_member(&directory.path().join("missing.toml"), "bake").is_err());

        write(&manifest, "[package]\nname = \"root\"\n");
        let mut permissions = fs::metadata(&manifest).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&manifest, permissions).unwrap();
        assert!(add_workspace_member(&manifest, "bake").is_err());
    }
}
