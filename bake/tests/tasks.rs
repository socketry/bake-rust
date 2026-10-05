// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

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

fn fake_cargo(directory: &TempDir) -> PathBuf {
    let source = directory.path().join("fake-cargo.rs");
    let path = directory.path().join(if cfg!(windows) {
        "fake-cargo.exe"
    } else {
        "fake-cargo"
    });
    std::fs::write(
        &source,
        r#"
fn main() {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>().join("\n") + "\n";
    std::fs::write(std::env::var_os("BAKE_TEST_CARGO_ARGUMENTS").unwrap(), arguments).unwrap();
    let exit_code: i32 = std::env::var("BAKE_TEST_CARGO_EXIT_CODE").unwrap().parse().unwrap();
    std::process::exit(exit_code);
}
"#,
    )
    .unwrap();
    let output = Command::new("rustc")
        .args(["--edition=2024", "--crate-name=fake_cargo"])
        .arg(&source)
        .arg("-o")
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "failed to compile fake cargo: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    path
}

fn run_with_cargo(arguments: &[&str], cargo: &Path, log: &Path, exit_code: u8) -> Output {
    Command::new(env!("CARGO_BIN_EXE_bake-rust-tasks"))
        .args(arguments)
        .current_dir(root())
        .env("BAKE_PROJECT_ROOT", root())
        .env("CARGO", cargo)
        .env("BAKE_TEST_CARGO_ARGUMENTS", log)
        .env("BAKE_TEST_CARGO_EXIT_CODE", exit_code.to_string())
        .output()
        .unwrap()
}

#[test]
fn checks_workspace_with_optional_offline_mode() {
    let directory = tempfile::tempdir().unwrap();
    let cargo = fake_cargo(&directory);
    let arguments = directory.path().join("arguments.txt");

    assert!(
        run_with_cargo(&["build:check"], &cargo, &arguments, 0)
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(&arguments).unwrap(),
        "check\n--workspace\n--locked\n"
    );

    assert!(
        run_with_cargo(&["build:check", "--offline", "true"], &cargo, &arguments, 0)
            .status
            .success()
    );
    assert_eq!(
        std::fs::read_to_string(&arguments).unwrap(),
        "check\n--workspace\n--locked\n--offline\n"
    );
}

#[test]
fn propagates_check_failures_and_runs_release_notes_hook() {
    let directory = tempfile::tempdir().unwrap();
    let cargo = fake_cargo(&directory);
    let arguments = directory.path().join("arguments.txt");
    let (heading, note) = release_section();

    let output = run_with_cargo(&["build:check"], &cargo, &arguments, 7);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo check failed"));

    let output = run_with_cargo(
        &["release:prepare", &heading, "--offline", "true"],
        &cargo,
        &arguments,
        7,
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("cargo check failed"));

    let cargo = fake_cargo(&directory);
    let output = stdout(run_with_cargo(
        &["release:prepare", &heading, "--offline", "true"],
        &cargo,
        &arguments,
        0,
    ));
    assert!(output.contains(&note));
}

fn non_utf8_argument() -> std::ffi::OsString {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![0xff])
    }

    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[0xd800])
    }
}

#[test]
fn rejects_non_utf8_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_bake-rust-tasks"))
        .arg(non_utf8_argument())
        .current_dir(root())
        .env("BAKE_PROJECT_ROOT", root())
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("task arguments must be UTF-8"));
}
