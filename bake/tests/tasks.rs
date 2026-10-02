// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
#[cfg(unix)]
use tempfile::TempDir;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[cfg(unix)]
fn release_section() -> (String, String) {
    let releases = std::fs::read_to_string(root().join("releases.md")).unwrap();
    let mut heading = None;

    for line in releases.lines() {
        if let Some(current_heading) = line.strip_prefix("## ") {
            heading = Some(current_heading.to_owned());
        } else if let (Some(heading), Some(note)) = (heading.as_ref(), line.strip_prefix("- ")) {
            return (heading.clone(), format!("- {note}"));
        }
    }

    panic!("releases.md does not contain a release section with notes");
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bake-rust-tasks"))
        .args(arguments)
        .current_dir(root())
        .env("BAKE_PROJECT_ROOT", root())
        .output()
        .unwrap()
}

fn stdout(output: Output) -> String {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn runs_project_tasks_and_formats_results() {
    assert_eq!(stdout(run(&["greet", "Ada"])), "Hello, Ada.\n");
    assert_eq!(
        stdout(run(&[
            "greet",
            "Samuel",
            "--excited",
            "true",
            "--labels",
            "Rust",
            "--labels",
            "concurrency",
        ])),
        "Hello, Samuel! [Rust, concurrency]\n"
    );
    assert_eq!(stdout(run(&["add", "20", "22"])), "42\n");

    let output = stdout(run(&["greet", "Ada", "::", "result"]));
    assert!(output.contains("\"previous\": \"Hello, Ada.\""));
}

#[test]
fn reports_task_errors_and_handles_help() {
    let output = run(&["add", "9223372036854775807", "1"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("addition overflow"));

    assert!(stdout(run(&["--help"])).contains("cargo:packages"));
    assert!(stdout(run(&["greet", "--help"])).contains("--excited value"));
}

#[test]
fn default_registry_exposes_the_builtin_tasks() {
    let registry = bake::Registry::default();
    let names: Vec<_> = registry.tasks().map(bake::Task::name).collect();

    assert!(names.contains(&"output"));
    assert!(names.contains(&"null"));
    assert!(registry.help(Some("missing")).is_err());
    if let Err(error) = bake::Registry::new().run() {
        assert!(error.to_string().contains("unknown task"));
    }
}

#[cfg(unix)]
fn fake_cargo(directory: &TempDir, exit_code: u8) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = directory.path().join("fake-cargo");
    std::fs::write(
        &path,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$BAKE_TEST_CARGO_ARGUMENTS\"\nexit {exit_code}\n"
        ),
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&path, permissions).unwrap();
    path
}

#[cfg(unix)]
fn run_with_cargo(arguments: &[&str], cargo: &Path, log: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bake-rust-tasks"))
        .args(arguments)
        .current_dir(root())
        .env("BAKE_PROJECT_ROOT", root())
        .env("CARGO", cargo)
        .env("BAKE_TEST_CARGO_ARGUMENTS", log)
        .output()
        .unwrap()
}

#[cfg(unix)]
#[test]
fn checks_workspace_with_optional_offline_mode() {
    let directory = tempfile::tempdir().unwrap();
    let cargo = fake_cargo(&directory, 0);
    let arguments = directory.path().join("arguments.txt");

    assert!(
        run_with_cargo(&["build:check"], &cargo, &arguments)
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(&arguments).unwrap(),
        "check\n--workspace\n--locked\n"
    );

    assert!(
        run_with_cargo(&["build:check", "--offline", "true"], &cargo, &arguments)
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(&arguments).unwrap(),
        "check\n--workspace\n--locked\n--offline\n"
    );
}

#[cfg(unix)]
#[test]
fn propagates_check_failures_and_runs_release_notes_hook() {
    let directory = tempfile::tempdir().unwrap();
    let cargo = fake_cargo(&directory, 7);
    let arguments = directory.path().join("arguments.txt");
    let (heading, note) = release_section();

    let output = run_with_cargo(&["build:check"], &cargo, &arguments);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo check failed"));

    let output = run_with_cargo(
        &["release:prepare", &heading, "--offline", "true"],
        &cargo,
        &arguments,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo check failed"));

    let cargo = fake_cargo(&directory, 0);
    let output = stdout(run_with_cargo(
        &["release:prepare", &heading, "--offline", "true"],
        &cargo,
        &arguments,
    ));
    assert!(output.contains(&note));
}

#[cfg(unix)]
#[test]
fn rejects_non_utf8_arguments() {
    use std::os::unix::ffi::OsStrExt;

    let output = Command::new(env!("CARGO_BIN_EXE_bake-rust-tasks"))
        .arg(std::ffi::OsStr::from_bytes(b"\xff"))
        .current_dir(root())
        .env("BAKE_PROJECT_ROOT", root())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("task arguments must be UTF-8"));
}
