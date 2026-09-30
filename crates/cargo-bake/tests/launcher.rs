// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const TASK_SOURCE: &str = r#"
fn main() {
    if std::env::args().any(|argument| argument == "exit-23") { std::process::exit(23); }
    println!("ROOT:{}", std::env::var("BAKE_PROJECT_ROOT").unwrap());
    println!("DIRECTORY:{}", std::env::current_dir().unwrap().canonicalize().unwrap().display());
    for argument in std::env::args().skip(1) { println!("ARG:{argument}"); }
}
"#;

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn project(task_directory: &str, workspace_configuration: &str) -> TempDir {
    let directory = tempfile::Builder::new()
        .prefix("bake project ")
        .tempdir()
        .unwrap();
    write(
        directory.path(),
        "Cargo.toml",
        &format!(
            "[workspace]\nmembers = [\"application\", \"{task_directory}\"]\nresolver = \"3\"\n{workspace_configuration}\n"
        ),
    );
    write(
        directory.path(),
        "application/Cargo.toml",
        "[package]\nname = \"application\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    );
    write(
        directory.path(),
        "application/src/lib.rs",
        "pub fn application() {}\n",
    );
    write(
        directory.path(),
        &format!("{task_directory}/Cargo.toml"),
        "[package]\nname = \"project-tasks\"\nversion = \"0.0.0\"\nedition = \"2024\"\npublish = false\n",
    );
    write(
        directory.path(),
        &format!("{task_directory}/src/main.rs"),
        TASK_SOURCE,
    );
    directory
}

fn launch(directory: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cargo-bake"))
        .args(arguments)
        .current_dir(directory)
        .env_remove("CARGO_TARGET_DIR")
        .env("CARGO_NET_OFFLINE", "true")
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn workspace_discovery_from_a_member_preserves_arguments_and_root() {
    let project = project("bake", "");
    let output = success(launch(
        &project.path().join("application/src"),
        &[
            "bake",
            "--offline",
            "greet",
            "a name with spaces",
            "--option",
            "value",
            "::",
            "second",
        ],
    ));
    let root = project.path().canonicalize().unwrap();
    assert!(output.contains(&format!("ROOT:{}\n", root.display())));
    assert!(output.contains(&format!("DIRECTORY:{}\n", root.display())));
    assert!(output.contains(
        "ARG:greet\nARG:a name with spaces\nARG:--option\nARG:value\nARG:::\nARG:second\n"
    ));
}

#[test]
fn manifest_option_and_workspace_metadata_override() {
    let project = project(
        "management",
        "[workspace.metadata.bake]\nmanifest = \"management/Cargo.toml\"",
    );
    let output = success(launch(
        project.path(),
        &[
            "--manifest-path",
            "application/Cargo.toml",
            "--offline",
            "--release",
            "--list",
        ],
    ));
    assert!(output.contains("ARG:--list\n"));
    assert!(project.path().join("target/release").is_dir());
}

#[test]
fn package_metadata_has_priority_and_uses_package_root() {
    let project = project(
        "bake",
        "[workspace.metadata.bake]\nmanifest = \"missing/Cargo.toml\"",
    );
    write(
        project.path(),
        "application/Cargo.toml",
        "[package]\nname = \"application\"\nversion = \"0.0.0\"\nedition = \"2024\"\n[package.metadata.bake]\nmanifest = \"../bake/Cargo.toml\"\n",
    );
    let output = success(launch(
        project.path(),
        &["--manifest-path", "application/Cargo.toml", "--offline"],
    ));
    assert!(output.contains(
        &format!("ROOT:{}\n", project.path().join("application").canonicalize().unwrap().display())
    ));
}

#[test]
fn standalone_task_workspace_is_supported() {
    let project = project("bake", "");
    write(
        project.path(),
        "Cargo.toml",
        "[workspace]\nmembers = [\"application\"]\nexclude = [\"bake\"]\nresolver = \"3\"\n",
    );
    write(
        project.path(),
        "bake/Cargo.toml",
        "[workspace]\n[package]\nname = \"separate-tasks\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
    );
    assert!(
        success(launch(project.path(), &["--offline", "--json", "task"]))
            .contains("ARG:--json\nARG:task\n")
    );
    assert!(project.path().join("bake/Cargo.lock").is_file());
}

#[test]
fn ambiguous_binary_requires_default_run() {
    let project = project("bake", "");
    write(
        project.path(),
        "bake/src/bin/other.rs",
        "fn main() { println!(\"OTHER\"); }\n",
    );
    let output = launch(project.path(), &["--offline"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("default-run"));
    write(
        project.path(),
        "bake/Cargo.toml",
        "[package]\nname = \"project-tasks\"\nversion = \"0.0.0\"\nedition = \"2024\"\ndefault-run = \"other\"\n",
    );
    assert_eq!(success(launch(project.path(), &["--offline"])), "OTHER\n");
}

#[test]
fn child_exit_code_is_preserved() {
    let project = project("bake", "");
    assert_eq!(
        launch(project.path(), &["--offline", "exit-23"])
            .status
            .code(),
        Some(23)
    );
}

#[test]
fn help_and_version_work_without_a_project() {
    let directory = tempfile::tempdir().unwrap();
    assert!(success(launch(directory.path(), &["bake", "--help"])).contains("Launcher options"));
    assert!(success(launch(directory.path(), &["--version"])).contains(env!("CARGO_PKG_VERSION")));
    assert!(!launch(directory.path(), &[]).status.success());
    assert!(
        !launch(directory.path(), &["--manifest-path"])
            .status
            .success()
    );
}

#[test]
fn missing_task_manifest_has_actionable_error() {
    let project = project("management", "");
    let output = launch(project.path(), &["--offline"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("create an unpublished bake/ binary crate")
    );
}

#[test]
fn locked_mode_does_not_create_a_lockfile() {
    let project = project("bake", "");
    let output = launch(project.path(), &["--locked", "--offline"]);
    assert!(!output.status.success());
    assert!(!project.path().join("Cargo.lock").exists());
    success(launch(project.path(), &["--offline"]));
    let lockfile = fs::read(project.path().join("Cargo.lock")).unwrap();
    success(launch(project.path(), &["--locked", "--offline"]));
    assert_eq!(
        fs::read(project.path().join("Cargo.lock")).unwrap(),
        lockfile
    );
}

#[test]
fn malformed_metadata_is_reported() {
    let project = project("bake", "[workspace.metadata.bake]\nmanifest = 42");
    let output = launch(project.path(), &["--offline"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must be a nonempty path string"));
}
