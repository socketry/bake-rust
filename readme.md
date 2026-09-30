# Bake for Rust

Write project tasks as ordinary Rust functions, then run them with `cargo bake`.
Task functions have typed arguments, generated help, explicit registration, and
a shared project context. Reusable task libraries are ordinary Cargo dependencies.

This is an initial implementation inspired by [Ruby Bake](https://github.com/ioquatix/bake),
[Bake Releases](https://github.com/ioquatix/bake-releases), and Cargo's
[xtask pattern](https://github.com/matklad/cargo-xtask).

## Try this repository

From a checkout, the included Cargo alias bootstraps the launcher:

```sh
cargo bake --list
cargo bake greet Samuel --excited --labels Rust
cargo bake greet --help
cargo bake add 20 22 :: result
cargo bake releases:notes Unreleased
```

The task crate is [bake/](bake/src/main.rs). Cargo compiles it on demand and caches
the build. `--offline` and `--locked` are available before the task name.

To install the launcher locally:

```sh
cargo install --path crates/cargo-bake --locked
```

The executable is `cargo-bake`; Cargo makes it available as `cargo bake`.
The package names below are prepared for publishing. This repository's initial
implementation does not imply that those packages have been published.

## Add tasks to a project

Add an unpublished `bake` binary crate to your workspace:

```text
Cargo.toml
src/
bake/
  Cargo.toml
  src/main.rs
```

In the project's `Cargo.toml`:

```toml
[workspace]
members = ["bake"]
```

In `bake/Cargo.toml`, use a path to your Bake checkout during development:

```toml
[package]
name = "project-tasks"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
bake = { package = "socketry-bake", path = "../../bake-rust" }
```

Adjust that path for your directory layout. Once published, a registry dependency
can use `version = "0.1"` instead of `path`.

In `bake/src/main.rs`:

```rust
use bake::{Registry, Result};

/// Greet someone, optionally with extra enthusiasm.
#[bake::task]
fn greet(name: String, #[bake(default = false)] excited: bool) -> Result<String> {
    Ok(format!("Hello, {name}{}", if excited { "!" } else { "." }))
}

fn main() -> Result<()> {
    let mut registry = Registry::new();
    registry.register(greet_task())?;
    registry.run()
}
```

`#[bake::task]` preserves `greet` and generates `greet_task()`, which describes
the arguments and adapts command-line input to the original function. Registering
tasks explicitly keeps imports, names, and composition visible in source code.

## Arguments and results

| Rust parameter | Command-line behavior |
| --- | --- |
| `name: String` | Required positional, also accepts `name=value` or `--name value` |
| `#[bake(named)] name: String` | Required named argument |
| `#[bake(default = 3)] count: usize` | Optional named argument with a typed default |
| `#[bake(default = "releases.md")] path: PathBuf` | String literal converted to the parameter type |
| `output: Option<PathBuf>` | Optional named argument, defaults to `None` |
| `labels: Vec<String>` | Repeatable named argument, defaults to an empty vector |
| `#[bake(default = false)] verbose: bool` | `--verbose`, `--verbose=false`, or `verbose=true` |
| `context: &mut Context` | Injected execution context, omitted from command-line arguments |

Values implement `FromStr`, with a displayable error. Custom argument types can
implement that trait. Defaults other than string literals must produce the
parameter's type. Defaults are evaluated when invoking the task. Parameter help
comes from `#[bake(help = "...")]`; task help comes from Rust documentation comments.
An explicitly marked `#[bake(context)]` parameter may have another name.

Named arguments also accept `--name=value`; flag names accept hyphens in place
of underscores. Use `--` before positional values that look like options or
contain `=`. `::` is reserved as a task separator; a named value can contain it
using `--name=::`. UTF-8 task arguments are required.

Task functions return `Result<Output, Error>` where `Output` implements
`serde::Serialize` and the error implements `Display`. `bake::Result` is a
convenience alias. The final task's result is printed: strings as text, other
values as JSON, and `()` silently. A leading `--json` formats the final result
as JSON, including strings and null. Tasks should use stderr for diagnostics
when callers need machine-readable stdout.

## Composition and hooks

Chain tasks with `::`. Bare task names also start a new task once the preceding
task's positional arguments are filled. Explicit separators make intent clearer:

```sh
cargo bake add 20 22 :: result
```

The entire chain is parsed and supplied values are type-checked before the first
task runs. Execution stops at the first error. Validation calls `FromStr` before
the adapter converts values again; argument parsers should be free of side effects.

Each invocation receives the same `Context`. It provides:

- `root()` — the project root determined by the launcher.
- `previous()` — the previous successful task's structured result.
- `insert`, `get`, `get_mut` — shared state indexed by Rust type.
- `call("task:name", &["argument=value"])` — invoke one task by its full registered name.
- `command("cargo")` — a `std::process::Command` configured to run in the project root.

Hooks are ordinary calls around an operation. For example, this repository's
`release:prepare` task calls `build:check`, then `releases:notes`. Direct Rust
function calls are also available when registry dispatch is unnecessary; they
do not automatically update `previous()`.

## Reusable task libraries

A library exports a function returning a registry:

```rust,ignore
let mut registry = bake::Registry::new();
registry.include("releases", bake_releases::registry()?)?;
```

The project chooses the namespace. Duplicate registrations are errors, and a
namespace import with collisions leaves the destination registry unchanged.
The included [release library](crates/releases/readme.md) provides:

```sh
cargo bake releases:notes Unreleased
cargo bake releases:update v0.1.0
cargo bake releases:notes v0.1.0 path=releases.md
```

`update` renames the `Unreleased` heading in the file. It does not change package
versions, commit, tag, or publish. Release headings use the documented ATX format
such as `## v0.1.0`.

## Discovery and configuration

The launcher uses `cargo metadata --format-version 1 --no-deps`. From a workspace
member it defaults to the workspace's `bake/Cargo.toml`. Override the path with:

```toml
[workspace.metadata.bake]
manifest = "development/Cargo.toml"
```

`[package.metadata.bake]` takes precedence for the selected package; its path and
execution root are relative to that package. Workspace configuration is relative
to the workspace root. `--manifest-path PATH` selects the **project** manifest.

The task package must have one binary, or select it with `package.default-run`.
It can belong to the project workspace, or be a separate workspace excluded from
the parent. A separate task workspace has its own dependency resolution and lockfile.

Launcher options (`--manifest-path`, `--offline`, `--locked`, `--release`) go before
the task name. Everything from the first task argument onward is forwarded intact.
`cargo bake --help` explains the launcher without compiling tasks; `--list` and
`TASK --help` compile and query the project's task registry. Child exit codes
are preserved. Process arguments are passed directly, without a shell.

## Packages

| Published package | Rust library / executable | Purpose |
| --- | --- | --- |
| `socketry-bake` | `bake` | Registry, arguments, context, task result handling |
| `socketry-bake-macros` | `socketry_bake_macros` | Function attribute, re-exported by `bake` |
| `socketry-cargo-bake` | `cargo-bake` | Project discovery and Cargo launcher |
| `socketry-bake-releases` | `bake_releases` | Reusable release-document tasks |

Tasks are synchronous in this initial implementation. An individual task can
start a runtime or a subprocess; Bake imposes no async runtime dependency.

See [conventions](conventions.md), [agent context](agent-context.md),
[design](design.md), and [release process](releasing.md).
