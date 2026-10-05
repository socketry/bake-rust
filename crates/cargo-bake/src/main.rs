// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod options;
mod project;

use options::Options;
use project::Project;
use std::ffi::OsString;
use std::process::{Command, ExitCode};

type Result<Output> = std::result::Result<Output, Box<dyn std::error::Error>>;

fn cargo() -> Command {
    Command::new(cargo_program(std::env::var_os("CARGO")))
}

fn cargo_program(program: Option<OsString>) -> OsString {
    program.unwrap_or_else(|| "cargo".into())
}

fn run() -> Result<i32> {
    run_with_arguments(std::env::args_os().skip(1), std::env::current_dir())
}

fn run_with_arguments(
    arguments: impl IntoIterator<Item = OsString>,
    current_directory: std::io::Result<std::path::PathBuf>,
) -> Result<i32> {
    let options = Options::parse(arguments)?;
    if options.help {
        println!(
            "cargo bake [OPTIONS] [TASK [ARGUMENTS] [:: TASK ...]]\n\nCompile and run the project's bake/ task crate.\n\nLauncher options (before TASK):\n  --manifest-path PATH  Project Cargo.toml (defaults to nearest ancestor)\n  --offline             Disable Cargo network access\n  --locked              Require unchanged Cargo.lock files\n  --release             Use Cargo's release profile\n  --regenerate          Create or refresh the private Bake task crate\n  --help                Show this launcher help without compiling tasks\n  --version             Show the launcher version\n\nTask options:\n  --list                List registered tasks\n  --json                Format the final task result as JSON\n  TASK --help           Show task arguments\n\nDefaults: workspace-root/bake/Cargo.toml. Override with\n[workspace.metadata.bake] or [package.metadata.bake] manifest = \"...\"."
        );
        return Ok(0);
    }
    if options.version {
        println!("cargo-bake {}", env!("CARGO_PKG_VERSION"));
        return Ok(0);
    }
    if options.regenerate && !options.arguments.is_empty() {
        return Err("--regenerate cannot be combined with task arguments".into());
    }
    run_project(options, current_directory)
}

fn run_project(
    options: Options,
    current_directory: std::io::Result<std::path::PathBuf>,
) -> Result<i32> {
    let mut command = cargo();
    run_project_with_command(options, current_directory, &mut command, command_status)
}

fn run_project_with_command(
    options: Options,
    current_directory: std::io::Result<std::path::PathBuf>,
    command: &mut Command,
    execute: impl FnOnce(&mut Command) -> Result<i32>,
) -> Result<i32> {
    match configure_project_command(options, current_directory, command)? {
        Some(code) => Ok(code),
        None => execute(command),
    }
}

fn configure_project_command(
    options: Options,
    current_directory: std::io::Result<std::path::PathBuf>,
    command: &mut Command,
) -> Result<Option<i32>> {
    let current_directory = current_directory?;
    if options.regenerate {
        Project::regenerate(&current_directory, &options)?;
        return Ok(Some(0));
    }
    let project = Project::discover(&current_directory, &options)?;
    command
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&project.task_manifest)
        .arg("--package")
        .arg(&project.package)
        .arg("--bin")
        .arg(&project.binary)
        .current_dir(&project.root)
        .env("BAKE_PROJECT_ROOT", &project.root);
    options.configure(command);
    if options.release {
        command.arg("--release");
    }
    command.arg("--").args(&options.arguments);
    Ok(None)
}

fn command_status(command: &mut Command) -> Result<i32> {
    let status = command.status()?;
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        Ok(status
            .code()
            .unwrap_or_else(|| 128 + status.signal().unwrap_or(1)))
    }
    #[cfg(not(unix))]
    Ok(status.code().unwrap_or(1))
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => {
            // Preserve the full process code, including Windows child exit codes.
            std::process::exit(code)
        }
        Err(error) => {
            eprintln!("cargo bake: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn cargo_program_uses_the_environment_or_default() {
        assert_eq!(cargo_program(None), "cargo");
        assert_eq!(cargo_program(Some("custom-cargo".into())), "custom-cargo");
        assert_eq!(
            cargo().get_program(),
            cargo_program(std::env::var_os("CARGO"))
        );
    }

    #[test]
    fn handles_help_version_and_invalid_regeneration_arguments_without_a_project() {
        let directory = tempfile::tempdir().unwrap();
        let current_directory = || Ok(directory.path().to_path_buf());

        assert_eq!(
            run_with_arguments(["--help"].map(OsString::from), current_directory()).unwrap(),
            0
        );
        assert_eq!(
            run_with_arguments(["--version"].map(OsString::from), current_directory()).unwrap(),
            0
        );
        assert!(
            run_with_arguments(
                ["--regenerate", "greet"].map(OsString::from),
                current_directory(),
            )
            .is_err()
        );
        assert!(
            run_with_arguments(
                ["--help"].map(OsString::from),
                Err(std::io::Error::other("no current directory")),
            )
            .is_ok()
        );
    }

    #[test]
    fn reports_project_discovery_and_regeneration_errors() {
        let directory = tempfile::tempdir().unwrap();
        assert!(run_project(Options::default(), Ok(directory.path().to_path_buf())).is_err());

        std::fs::write(
            directory.path().join("Cargo.toml"),
            "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2024\"\n[package.metadata.bake]\nmanifest = \"\"\n",
        )
        .unwrap();
        assert!(
            run_project(
                Options {
                    regenerate: true,
                    ..Options::default()
                },
                Ok(directory.path().to_path_buf()),
            )
            .is_err()
        );

        assert!(
            run_project(
                Options::default(),
                Err(std::io::Error::other("no current directory")),
            )
            .is_err()
        );
    }

    #[test]
    fn reports_child_process_start_failures() {
        let directory = tempfile::tempdir().unwrap();
        let mut command = Command::new(PathBuf::from(directory.path()).join("missing-cargo"));

        assert!(command_status(&mut command).is_err());
    }

    #[test]
    fn configures_the_task_command_and_returns_regeneration_results() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir_all(root.join("bake/src")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"bake\"]\nresolver = \"3\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("bake/Cargo.toml"),
            "[package]\nname = \"example-bake\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(root.join("bake/src/main.rs"), "fn main() {}\n").unwrap();

        let canonical_root = root.canonicalize().unwrap();
        let canonical_manifest = root.join("bake/Cargo.toml").canonicalize().unwrap();
        let mut command = Command::new("fake-cargo");
        let options = Options {
            offline: true,
            locked: true,
            release: true,
            arguments: ["greet", "Ada"].map(OsString::from).into(),
            ..Options::default()
        };
        let status =
            run_project_with_command(options, Ok(root.to_path_buf()), &mut command, |command| {
                assert_eq!(
                    command
                        .get_args()
                        .map(|argument| argument.to_os_string())
                        .collect::<Vec<_>>(),
                    [
                        "run",
                        "--quiet",
                        "--manifest-path",
                        canonical_manifest.to_str().unwrap(),
                        "--package",
                        "example-bake",
                        "--bin",
                        "example-bake",
                        "--offline",
                        "--locked",
                        "--release",
                        "--",
                        "greet",
                        "Ada",
                    ]
                    .map(OsString::from)
                    .to_vec()
                );
                assert_eq!(command.get_current_dir(), Some(canonical_root.as_path()));
                Ok(23)
            })
            .unwrap();
        assert_eq!(status, 23);

        assert_eq!(
            run_project_with_command(
                Options {
                    regenerate: true,
                    ..Options::default()
                },
                Ok(root.to_path_buf()),
                &mut Command::new("fake-cargo"),
                command_status,
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn command_status_preserves_successful_child_exit_codes() {
        #[cfg(unix)]
        let mut command = {
            let mut command = Command::new("sh");
            command.args(["-c", "exit 23"]);
            command
        };
        #[cfg(windows)]
        let mut command = {
            let mut command = Command::new("cmd.exe");
            command.args(["/C", "exit 23"]);
            command
        };

        assert_eq!(command_status(&mut command).unwrap(), 23);
    }
}
