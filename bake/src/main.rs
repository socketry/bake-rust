// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use bake::{Context, Error, Registry, Result, Value};
// Pull the dependency into the executable so its task descriptors are linked.
use bake_agent_context as _;
use bake_cargo as _;
use bake_license as _;
use bake_releases as _;
use bake_test_rust as _;
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
        &["--offline", if offline { "true" } else { "false" }],
    )?;
    context.call("releases:notes", &[&version])
}

fn run() -> Result<()> {
    Registry::discover()?.run()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_project_and_dependency_tasks_automatically() {
        let registry = Registry::discover().unwrap();
        let names: Vec<_> = registry.tasks().map(|task| task.name()).collect();

        assert!(names.contains(&"greet"));
        assert!(names.contains(&"build:check"));
        assert!(names.contains(&"releases:notes"));
        assert!(names.contains(&"releases:update"));
        assert!(names.contains(&"cargo:after_version_bump"));
        assert!(names.contains(&"output"));
        assert!(names.contains(&"null"));
    }
}

#[path = "bake_generated_tasks/mod.rs"]
mod bake_generated_tasks;
