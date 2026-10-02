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
    let mut command = cargo();
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
    Ok(serde_json::from_slice(&output.stdout)?)
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
        let workspace_root = project_metadata.workspace_root.canonicalize()?;
        let (root, relative_manifest) = if let Some(path) = package_manifest {
            (
                manifest
                    .parent()
                    .ok_or("manifest has no parent directory")?
                    .to_path_buf(),
                path,
            )
        } else {
            (
                project_metadata.workspace_root.clone(),
                configured_manifest(&project_metadata.metadata)?.unwrap_or("bake/Cargo.toml"),
            )
        };
        let root = root.canonicalize()?;
        let task_manifest = root.join(relative_manifest);

        Ok(Location {
            root,
            workspace_manifest: workspace_root.join("Cargo.toml"),
            workspace_root,
            task_manifest,
        })
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
        let created = !location.task_manifest.is_file();
        if created {
            bootstrap_task_package(&location)?;
        }

        let task_manifest = location.task_manifest.canonicalize()?;
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
            .ok_or("binary source path has no parent directory")?
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

fn workspace_manifest_for(manifest: &Path, document: &DocumentMut) -> Result<PathBuf> {
    if document.as_table().contains_key("workspace") {
        return Ok(manifest.to_path_buf());
    }

    if let Some(workspace_path) = table_item(document.as_table().get("package"), "workspace")
        .and_then(|item| item.as_value())
        .and_then(TomlValue::as_str)
    {
        let parent = manifest
            .parent()
            .ok_or("manifest has no parent directory")?;
        return Ok(parent
            .join(workspace_path)
            .join("Cargo.toml")
            .canonicalize()?);
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
            return Ok(candidate.canonicalize()?);
        }
    }

    Ok(manifest.to_path_buf())
}

fn locate_from_manifest(manifest: &Path) -> Result<Location> {
    let contents = fs::read_to_string(manifest)?;
    let document = contents.parse::<DocumentMut>()?;
    let workspace_manifest = workspace_manifest_for(manifest, &document)?;
    let workspace_contents = fs::read_to_string(&workspace_manifest)?;
    let workspace_document = workspace_contents.parse::<DocumentMut>()?;
    let workspace_root = workspace_manifest
        .parent()
        .ok_or("workspace manifest has no parent directory")?
        .canonicalize()?;
    let package_root = manifest
        .parent()
        .ok_or("manifest has no parent directory")?
        .canonicalize()?;
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
        remove_legacy_generated_module(source_path)?;
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
    const GENERATED_MARKER: &str = "// Generated by `cargo bake --regenerate`; do not edit.";

    let Some(source_directory) = source_path.parent() else {
        return Ok(());
    };
    let legacy_directory = source_directory.join("__bake_generated_tasks");
    let legacy_source = legacy_directory.join("mod.rs");
    if fs::read_to_string(&legacy_source).is_ok_and(|source| source.starts_with(GENERATED_MARKER)) {
        fs::remove_file(legacy_source)?;
        let _ = fs::remove_dir(legacy_directory);
    }
    Ok(())
}

fn bootstrap_task_package(location: &Location) -> Result<()> {
    let task_directory = location
        .task_manifest
        .parent()
        .ok_or("task manifest has no parent directory")?;
    fs::create_dir_all(task_directory)?;
    let task_directory = task_directory.canonicalize()?;

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
