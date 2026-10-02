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
    let options = Options::parse(std::env::args_os().skip(1))?;
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
    if options.regenerate {
        if !options.arguments.is_empty() {
            return Err("--regenerate cannot be combined with task arguments".into());
        }
        Project::regenerate(&std::env::current_dir()?, &options)?;
        return Ok(0);
    }
    let project = Project::discover(&std::env::current_dir()?, &options)?;
    let mut command = cargo();
    command
        .args(["run", "--quiet", "--manifest-path"])
        .arg(&project.task_manifest)
        .arg("--package")
        .arg(&project.package)
        .arg("--bin")
        .arg(&project.binary)
        .current_dir(&project.root)
        .env("BAKE_PROJECT_ROOT", &project.root);
    options.configure(&mut command);
    if options.release {
        command.arg("--release");
    }
    let status = command.arg("--").args(&options.arguments).status()?;
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

    #[test]
    fn cargo_program_uses_the_environment_or_default() {
        assert_eq!(cargo_program(None), "cargo");
        assert_eq!(cargo_program(Some("custom-cargo".into())), "custom-cargo");
    }
}
