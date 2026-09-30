# Bake for Rust

Write project tasks as ordinary Rust functions, then run them with `cargo bake`.
Task functions have typed arguments, generated help, automatic discovery, and a
shared project context. Reusable task libraries are ordinary Cargo dependencies.

This is an initial implementation inspired by [Ruby Bake](https://github.com/ioquatix/bake),
[Bake Releases](https://github.com/ioquatix/bake-releases), and Cargo's
[xtask pattern](https://github.com/matklad/cargo-xtask).

## Try this repository

From a checkout, the included Cargo alias bootstraps the launcher:

```sh
cargo bake --list
cargo bake greet Samuel --excited true --labels Rust
cargo bake greet --help
cargo bake add 20 22 :: result
cargo bake releases:notes Unreleased
cargo bake cargo:packages
cargo bake license:update
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
    Registry::discover()?.run()
}
```

`#[bake::task]` preserves `greet` and generates `greet_task()`, which describes
the arguments and adapts command-line input to the original function. It also adds
the descriptor to Bake's link-time registration table. `Registry::discover()`
collects tasks from the executable and linked task libraries. Nested Rust modules
form namespaces, so a function in `releases::` becomes `releases:notes`.

## Arguments and results

| Rust parameter | Command-line behavior |
| --- | --- |
| `name: String` | Required positional, also accepts `--name value` |
| `#[bake(named)] name: String` | Required named argument |
| `#[bake(default = 3)] count: usize` | Optional named argument with a typed default |
| `#[bake(default = "releases.md")] path: PathBuf` | String literal converted to the parameter type |
| `output: Option<PathBuf>` | Optional named argument, defaults to `None` |
| `labels: Vec<String>` | Repeatable named argument, defaults to an empty vector |
| `#[bake(default = false)] verbose: bool` | `--verbose true` or `--verbose false` |
| `context: &mut Context` | Injected execution context, omitted from command-line arguments |
| `#[bake(input)] input: Value` | Injected result from the preceding task in a chain |

Values implement `FromStr`, with a displayable error. Custom argument types can
implement that trait. Defaults other than string literals must produce the
parameter's type. Defaults are evaluated when invoking the task. Parameter help
comes from `#[bake(help = "...")]`; task help comes from Rust documentation comments.
An explicitly marked `#[bake(context)]` parameter may have another name.

Named arguments use two tokens: `--name value`. This also applies to boolean and
repeatable arguments. Equals signs are not a named-argument separator; flag names
accept hyphens in place of underscores. Use `--` before positional values that
look like options. `::` is reserved as a task separator. UTF-8 task arguments are
required.

Task functions return `Result<Output, Error>` where `Output` implements
`serde::Serialize` and the error implements `Display`. `bake::Result` is a
convenience alias. After the final task, Bake invokes its registered `output`
task unless that task handled output itself. The default `output` task prints
strings as text, structured values as pretty JSON, and `()` silently. A leading
`--json` selects JSON, including for strings and null. Tasks should use stderr
for diagnostics when callers need machine-readable stdout.

The built-in `output` task also works in a chain. Its input is the previous
task's result, and it returns that result for further processing:

```sh
cargo bake greet Samuel output --format json
cargo bake releases:notes Unreleased output --file notes.txt
```

Use `--format raw`, `--format json`, or `--format ndjson`; JSON and NDJSON file
extensions also select a format. Raw text is the default for other file extensions.
Output files are relative to the project root, and their parent directories must exist.
The `null` task consumes a result without printing it. Mark a custom task with
`#[bake::task(output)]` if it handles output, or replace the default formatter
with `registry.replace("output", custom_output_task())`.

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
- `call("task:name", &["--argument", "value"])` — invoke one task by its full registered name.
- `call_if_registered("task:name", &[...])` — invoke an optional task,
  returning `None` if it is not registered.
- `command("cargo")` — a `std::process::Command` configured to run in the project root.

Hooks are ordinary calls around an operation. For example, this repository's
`release:prepare` task calls `build:check`, then `releases:notes`. Direct Rust
function calls are also available when registry dispatch is unnecessary; they
do not automatically update `previous()`.

Reusable tasks can invoke project-local hooks through the shared registry. The
`bake-cargo` version tasks optionally call `cargo:after_version_bump`, passing
the new workspace version. A project can define that task in its private
`bake/` crate; if it is absent, the version bump continues without a hook.

## Reusable task libraries

Task functions in a library are discovered with the same attribute. Put them in
a semantic module to give them a namespace:

```rust,ignore
pub mod releases {
    #[bake::task]
    pub fn notes(/* typed arguments */) -> bake::Result<String> {
        // ...
    }
}
```

Add the library as a Cargo dependency and reference it from the task binary so
Rust includes its registration entries in the link:

```rust,ignore
use bake_releases as _;

bake::Registry::discover()?.run()
```

This removes per-task registration and namespace boilerplate. Explicit
`Registry::register` and `Registry::include` remain available when a project needs
to assemble names dynamically. Duplicate discovered names are errors. The
[Bake Releases](https://github.com/socketry/bake-releases-rust) library provides:

```sh
cargo bake releases:notes Unreleased
cargo bake releases:update v0.1.0
cargo bake releases:notes v0.1.0 --path releases.md
```

`update` renames the `Unreleased` heading in the file. It does not change package
versions, commit, tag, or publish. Release headings use the documented ATX format
such as `## v0.1.0`.

The companion [Bake Cargo](https://github.com/socketry/bake-cargo-rust) library
provides Cargo workspace tasks, GitHub release creation, publishing workflow
generation, GitHub release protections, and crates.io trusted publishers. Its
shared version tasks optionally invoke `cargo:after_version_bump` with the new
version. This repository defines the hook to run `license:update` and
`releases:update`. `cargo:release` validates and packages a candidate for a
reviewed release pull request. After the pull request merges, the workflow waits
for the `crates-io` environment approval, publishes the workspace through
trusted publishing, and creates the `vVERSION` tag after all uploads succeed.
The initial publish can be followed by
trusted-publisher setup with the explicit `cargo:bootstrap PACKAGE` task. Review
its effects and package contents before invoking it.

The separately reusable [Bake License](https://github.com/socketry/bake-license-rust)
library tracks Git authorship, refreshes `license.md`, removes the README License
section, and updates Rust source copyright headers.

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
Options that take values use a separate following argument.

The task package must have one binary, or select it with `package.default-run`.
It can belong to the project workspace, or be a separate workspace excluded from
the parent. A separate task workspace has its own dependency resolution and lockfile.

Launcher options (`--manifest-path PATH`, `--offline`, `--locked`, `--release`) go
before the task name. Everything from the first task argument onward is forwarded intact.
`cargo bake --help` explains the launcher without compiling tasks; `--list` and
`TASK --help` compile and query the project's task registry. Child exit codes
are preserved. Process arguments are passed directly, without a shell.

## Packages

| Published package | Rust library / executable | Purpose |
| --- | --- | --- |
| `socketry-bake` | `bake` | Registry, arguments, context, task result handling |
| `bake-macros` | `bake_macros` | Function attribute, re-exported by `bake` |
| `socketry-cargo-bake` | `cargo-bake` | Project discovery and Cargo launcher |
| `bake-releases` | `bake_releases` | Release-document tasks ([repository](https://github.com/socketry/bake-releases-rust)) |
| `bake-cargo` | `bake_cargo` | Cargo project and release tasks ([repository](https://github.com/socketry/bake-cargo-rust)) |
| `bake-license` | `bake_license` | License and copyright maintenance tasks ([repository](https://github.com/socketry/bake-license-rust)) |
| `bake-agent-context` | `bake_agent_context` | Dependency context tasks ([repository](https://github.com/socketry/bake-agent-context-rust)) |

For local development of the task binary, check out the task repositories beside
this repository as `../bake-releases-rust`, `../bake-cargo-rust`, and
`../bake-license-rust`.

Tasks are synchronous in this initial implementation. An individual task can
start a runtime or a subprocess; Bake imposes no async runtime dependency.

## Context

This crate includes [development context](context/development.md), a
[design overview](context/design.md), and a guide to
[structuring and using task libraries](context/task-libraries.md). The local
task executable also includes Bake Agent Context, so run
`cargo bake agent:context:install` to install context from its dependencies.
The generated `.agents/context/` directory is ignored by Git.

For repository-only conventions, see
[.agents/conventions.md](https://github.com/socketry/bake-rust/blob/main/.agents/conventions.md).
For the release process, see
[.agents/releasing.md](https://github.com/socketry/bake-rust/blob/main/.agents/releasing.md).
