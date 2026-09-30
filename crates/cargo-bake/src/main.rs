mod options;
mod project;

use options::Options;
use project::Project;
use std::process::{Command, ExitCode};

type Result<Output> = std::result::Result<Output, Box<dyn std::error::Error>>;

fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

fn run() -> Result<i32> {
    let options = Options::parse(std::env::args_os().skip(1))?;
    if options.help {
        println!(
            "cargo bake [OPTIONS] [TASK [ARGUMENTS] [:: TASK ...]]\n\nCompile and run the project's bake/ task crate.\n\nLauncher options (before TASK):\n  --manifest-path PATH  Project Cargo.toml (defaults to nearest ancestor)\n  --offline             Disable Cargo network access\n  --locked              Require unchanged Cargo.lock files\n  --release             Use Cargo's release profile\n  --help                Show this launcher help without compiling tasks\n  --version             Show the launcher version\n\nTask options:\n  --list                List registered tasks\n  --json                Format the final task result as JSON\n  TASK --help           Show task arguments\n\nDefaults: workspace-root/bake/Cargo.toml. Override with\n[workspace.metadata.bake] or [package.metadata.bake] manifest = \"...\"."
        );
        return Ok(0);
    }
    if options.version {
        println!("cargo-bake {}", env!("CARGO_PKG_VERSION"));
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
