use crate::{Result, cargo, options::Options};
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;

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
}

#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
}

pub(crate) struct Project {
    pub(crate) root: PathBuf,
    pub(crate) task_manifest: PathBuf,
    pub(crate) package: String,
    pub(crate) binary: String,
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
        )
        .stderr(Stdio::inherit());
    options.configure(&mut command);
    let output = command.output()?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed for {} ({})",
            manifest.display(),
            output.status
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
    pub(crate) fn discover(directory: &Path, options: &Options) -> Result<Self> {
        let manifest = match &options.manifest {
            Some(path) => directory.join(path),
            None => directory
                .ancestors()
                .map(|parent| parent.join("Cargo.toml"))
                .find(|candidate| candidate.is_file())
                .ok_or("no Cargo.toml found; run inside a Cargo project or use --manifest-path")?,
        }
        .canonicalize()?;
        let project_metadata = metadata(&manifest, options)?;
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
        let task_manifest = root.join(relative_manifest).canonicalize().map_err(|error| {
            format!("cannot open task manifest {}: {error}; create an unpublished bake/ binary crate and add it to the workspace", root.join(relative_manifest).display())
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
        let binaries: Vec<_> = package
            .targets
            .iter()
            .filter(|target| target.kind.iter().any(|kind| kind == "bin"))
            .collect();
        let binary = if let Some(default_run) = &package.default_run {
            binaries
                .iter()
                .find(|target| &target.name == default_run)
                .ok_or("task package default-run must name a binary target")?
                .name
                .clone()
        } else if let [binary] = binaries.as_slice() {
            binary.name.clone()
        } else {
            return Err(
                "task package must have one binary, or select one with package.default-run".into(),
            );
        };
        Ok(Self {
            root,
            task_manifest,
            package: package.name,
            binary,
        })
    }
}
