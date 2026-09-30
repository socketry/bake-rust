use bake::{Context, Error, Registry, Result, Value};
use std::process::ExitCode;

/// Greet someone using typed arguments, defaults, and repeatable labels.
#[bake::task]
fn greet(
    name: String,
    #[bake(default = false)] excited: bool,
    #[bake(help = "Repeat --labels to add more than one.")] labels: Vec<String>,
) -> Result<String> {
    let punctuation = if excited { "!" } else { "." };
    let labels = if labels.is_empty() {
        String::new()
    } else {
        format!(" [{}]", labels.join(", "))
    };
    Ok(format!("Hello, {name}{punctuation}{labels}"))
}

/// Add two signed integers and return a structured numeric result.
#[bake::task]
fn add(left: i64, right: i64) -> Result<i64> {
    left.checked_add(right)
        .ok_or_else(|| Error::new("addition overflow"))
}

/// Wrap the preceding task's result, demonstrating task-chain composition.
#[bake::task]
fn result(context: &mut Context) -> Result<Value> {
    bake::value(std::collections::BTreeMap::from([(
        "previous",
        context.previous().clone(),
    )]))
}

/// Check all workspace crates with Cargo. Arguments never pass through a shell.
#[bake::task(name = "build:check")]
fn check(context: &mut Context, #[bake(default = false)] offline: bool) -> Result<()> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut command = context.command(cargo);
    command.args(["check", "--workspace", "--locked"]);
    if offline {
        command.arg("--offline");
    }
    let status = command.status()?;
    if status.success() {
        Ok(())
    } else {
        Err(Error::new(format!("cargo check failed: {status}")))
    }
}

/// Example release hook: run the project check before returning release notes.
#[bake::task(name = "release:prepare")]
fn prepare(
    context: &mut Context,
    version: String,
    #[bake(default = false)] offline: bool,
) -> Result<Value> {
    context.call(
        "build:check",
        &[if offline {
            "offline=true"
        } else {
            "offline=false"
        }],
    )?;
    context.call("releases:notes", &[&version])
}

fn run() -> Result<()> {
    let mut registry = Registry::new();
    registry
        .register(greet_task())?
        .register(add_task())?
        .register(result_task())?
        .register(check_task())?
        .register(prepare_task())?;
    registry.include("releases", bake_releases::registry()?)?;
    registry.run()
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bake: {error}");
            ExitCode::FAILURE
        }
    }
}
