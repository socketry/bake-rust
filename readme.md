# `bake`

Write project tasks as ordinary Rust functions, then run them with `cargo bake`. Task functions have typed arguments, generated help, automatic discovery, and a shared project context. Reusable task libraries are ordinary Cargo dependencies.

This is an initial implementation inspired by [Ruby Bake](https://github.com/ioquatix/bake), [Bake Releases](https://github.com/ioquatix/bake-releases), and Cargo's [xtask pattern](https://github.com/matklad/cargo-xtask).

## Try this repository

Install the `socketry-cargo-bake` launcher from this checkout, then run the project tasks:

```sh
cargo install --path crates/cargo-bake --locked
cargo bake --list
cargo bake greet Samuel --excited true --labels Rust
cargo bake greet --help
cargo bake add 20 22 :: result
cargo bake releases:notes Unreleased
cargo bake cargo:packages
cargo bake license:update
```

The task crate is [bake/](bake/src/main.rs). Cargo compiles it on demand and caches the build. `--offline` and `--locked` are available before the task name.

The executable is `cargo-bake`; Cargo makes it available as `cargo bake`. The core library package is `bake`, and the launcher package is `socketry-cargo-bake`. To install the published launcher:

```sh
cargo install socketry-cargo-bake
```

The earlier `socketry-bake` package remains available for existing projects; use `bake` for new projects.

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

In `bake/Cargo.toml`, depend on Bake by its published crate version:

```toml
[package]
name = "project-tasks"
version = "0.0.0"
edition = "2024"
publish = false

[dependencies]
bake = "0.19"
```

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

`#[bake::task]` preserves `greet` and generates `greet_task()`, which describes the arguments and adapts command-line input to the original function. It also adds the descriptor to Bake's link-time registration table. `Registry::discover()` collects tasks from the executable and linked task libraries. Nested Rust modules form namespaces by default. Library crate names beginning with `bake_` supply a prefix too: `bake_releases::notes` registers as `releases:notes`. A fully qualified name such as `#[bake::task(name = "releases:notes")]` overrides inference. Project binary targets keep their existing module-based names. Names are resolved at compile time, so generated descriptors contain the same final names whether registered manually or collected by `Registry::discover()`.

## Arguments and results

| Rust parameter | Command-line behavior |
| --- | --- |
| `name: String` | Required positional, also accepts `--name value` |
| `#[bake(named)] name: String` | Required named argument |
| `#[bake(default = 3)] count: usize` | Optional named argument with a typed default |
| `#[bake(default = "releases.md")] path: PathBuf` | String literal converted to the parameter type |
| `output: Option<PathBuf>` | Optional named argument, defaults to `None` |
| `labels: Vec<String>` | Repeatable named argument, defaults to an empty vector |
| `#[bake(positional)] paths: Vec<PathBuf>` | Variadic positional arguments, consumed through `::` or the end of the command |
| `#[bake(default = false)] verbose: bool` | `--verbose true` or `--verbose false` |
| `context: &mut Context` | Injected execution context, omitted from command-line arguments |
| `#[bake(input)] input: Value` | Injected result from the preceding task in a chain |

Values implement `FromStr`, with a displayable error. Custom argument types can implement that trait. Defaults other than string literals must produce the parameter's type. Defaults are evaluated when invoking the task. Parameter help comes from `#[bake(help = "...")]`; task help comes from Rust documentation comments. An explicitly marked `#[bake(context)]` parameter may have another name.

Named arguments use two tokens: `--name value`. This also applies to boolean and repeatable arguments. Equals signs are not a named-argument separator; flag names accept hyphens in place of underscores. Use `--` before positional values that look like options. `::` is reserved as a task separator. UTF-8 task arguments are required.

By default, `Vec<T>` parameters are repeatable named options. Add `#[bake(positional)]` to make a `Vec<T>` consume bare positional values instead. It must be the last positional parameter in its task. Since it cannot infer where a following task starts, use `::` before another task:

```sh
cargo bake files:normalize path/one.md path/two.md :: output
```

Task functions return `Result<Output, Error>` where `Output` implements `serde::Serialize` and the error implements `Display`. `bake::Result` is a convenience alias. After the final task, Bake invokes its registered `output` task unless that task handled output itself. The default `output` task prints strings as text, structured values as pretty JSON, and `()` silently. A leading `--json` selects JSON, including for strings and null. Tasks should use stderr for diagnostics when callers need machine-readable stdout.

The built-in `output` task also works in a chain. Its input is the previous task's result, and it returns that result for further processing:

```sh
cargo bake greet Samuel output --format json
cargo bake releases:notes Unreleased output --file notes.txt
```

Use `--format raw`, `--format json`, or `--format ndjson`; JSON and NDJSON file extensions also select a format. Raw text is the default for other file extensions. Output files are relative to the project root, and their parent directories must exist. The `null` task consumes a result without printing it. Mark a custom task with `#[bake::task(output)]` if it handles output, or replace the default formatter with `registry.replace("output", custom_output_task())`.

## Composition and hooks

Chain tasks with `::`. Bare task names also start a new task once the preceding task's positional arguments are filled. An unbounded positional `Vec<T>` consumes all bare arguments through the end of its invocation, so an explicit `::` is required before another task. Explicit separators make intent clearer:

```sh
cargo bake add 20 22 :: result
```

The entire chain is parsed and supplied values are type-checked before the first task runs. Execution stops at the first error. Validation calls `FromStr` before the adapter converts values again; argument parsers should be free of side effects.

Each invocation receives the same `Context`. It provides:

- `root()` — the project root determined by the launcher.
- `previous()` — the previous successful task's structured result.
- `insert`, `get`, `get_mut` — shared state indexed by Rust type.
- `call("task:name", &["--argument", "value"])` — invoke one task by its full registered name.
- `call_if_registered("task:name", &[...])` — invoke an optional task, returning `None` if it is not registered.
- `command("cargo")` — a `std::process::Command` configured to run in the project root.

Hooks are ordinary calls around an operation. For example, this repository's `release:prepare` task calls `build:check`, then `releases:notes`. Direct Rust function calls are also available when registry dispatch is unnecessary; they do not automatically update `previous()`.

Reusable tasks can invoke project-local hooks through the shared registry. The `bake-cargo` version tasks optionally call `cargo:after_version_bump`, passing the new workspace version. A project can define that task in its private `bake/` crate; if it is absent, the version bump continues without a hook.

## Reusable task libraries

Give the library a semantic Rust API, then expose its operations with the task attribute. Small adapters can resolve project-relative paths and translate command arguments. For example, a `bake_releases` library with a `read_notes` operation could expose a task directly from its crate root:

```rust,ignore
// src/lib.rs
#[bake::task]
pub fn notes(
    context: &mut bake::Context,
    version: String,
    #[bake(default = "releases.md")] path: std::path::PathBuf,
) -> bake::Result<String> {
    read_notes(&context.root().join(path), &version)
}
```

Keep the main Rust API accessible at the crate root, with modules for meaningful domain concepts. This function would be called as `bake_releases::notes` in Rust and `releases:notes` through Bake. Bake removes the leading `bake_` and converts remaining crate-name underscores to colons, so `bake_agent_context` supplies `agent:context`. Nested modules extend that namespace, with module-name underscores converted to hyphens. A matching namespace already expressed by wrapper modules is included only once. An operation whose signature already suits Bake can be annotated directly.

Explicit names retain their existing behavior: `name = "namespace:task"` specifies the entire name, while a short name uses only the defining module's namespace. This lets libraries preserve names such as a root `test` command. See the [migration guidance](context/task-libraries.md#align-an-existing-library) for existing library tasks whose default names gain a crate prefix.

Add the library as a Cargo dependency and reference it from the task binary so Rust includes its registration entries in the link:

```rust,ignore
use bake_releases as _;

bake::Registry::discover()?.run()
```

This removes per-task registration and namespace boilerplate. Explicit `Registry::register` and `Registry::include` remain available when a project needs to assemble names dynamically. Duplicate discovered names are errors. The [Bake Releases](https://github.com/socketry/bake-releases-rust) library provides:

```sh
cargo bake releases:notes Unreleased
cargo bake releases:update v0.1.0
cargo bake releases:notes v0.1.0 --path releases.md
```

`update` renames the `Unreleased` heading in the file. It does not change package versions, commit, tag, or publish. Release headings use the documented ATX format such as `## v0.1.0`.

The companion [Bake Cargo](https://github.com/socketry/bake-cargo-rust) library provides Cargo workspace tasks, GitHub release creation, publishing workflow generation, GitHub release protections, and crates.io trusted publishers. Its shared version tasks optionally invoke `cargo:after_version_bump` with the new version. Socketry projects use `socketry-project` in their private task package to supply the shared hook for license, release-note, and readme updates. `cargo:release` validates and packages a candidate for a reviewed release pull request. After the pull request merges, the workflow waits for the `crates-io` environment approval, publishes the workspace through trusted publishing, and creates the `vVERSION` tag after all uploads succeed. The initial publish can be followed by trusted-publisher setup with the explicit `cargo:bootstrap PACKAGE` task. Review its effects and package contents before invoking it.

The separately reusable [Bake License](https://github.com/socketry/bake-license-rust) library tracks Git authorship, refreshes `license.md`, removes the README License section, and updates Rust source copyright headers.

See the [task library guide](context/task-libraries.md) for more details on structuring and using reusable task libraries.

## Discovery and configuration

The launcher uses `cargo metadata --format-version 1 --no-deps`. From a workspace member it defaults to the workspace's `bake/Cargo.toml`. Override the path with:

```toml
[workspace.metadata.bake]
manifest = "development/Cargo.toml"
```

`[package.metadata.bake]` takes precedence for the selected package; its path and execution root are relative to that package. Workspace configuration is relative to the workspace root. `--manifest-path PATH` selects the **project** manifest. Options that take values use a separate following argument.

The task package must have one binary, or select it with `package.default-run`. It can belong to the project workspace, or be a separate workspace excluded from the parent. A separate task workspace has its own dependency resolution and lockfile.

Launcher options (`--manifest-path PATH`, `--offline`, `--locked`, `--release`) go before the task name. Everything from the first task argument onward is forwarded intact. `cargo bake --help` explains the launcher without compiling tasks; `--list` and `TASK --help` compile and query the project's task registry. Child exit codes are preserved. Process arguments are passed directly, without a shell.

## Packages

| Published package | Rust library / executable | Purpose |
| --- | --- | --- |
| `bake` | `bake` | Registry, arguments, context, task result handling |
| `bake-macros` | `bake_macros` | Function attribute, re-exported by `bake` |
| `socketry-cargo-bake` | `cargo-bake` | Project discovery and Cargo launcher |
| `bake-releases` | `bake_releases` | Release-document tasks ([repository](https://github.com/socketry/bake-releases-rust)) |
| `bake-cargo` | `bake_cargo` | Cargo project and release tasks ([repository](https://github.com/socketry/bake-cargo-rust)) |
| `bake-license` | `bake_license` | License and copyright maintenance tasks ([repository](https://github.com/socketry/bake-license-rust)) |
| `bake-agent-context` | `bake_agent_context` | Dependency context tasks ([repository](https://github.com/socketry/bake-agent-context-rust)) |

For local development of the task binary, check out the task repositories beside this repository as `../bake-releases-rust`, `../bake-cargo-rust`, and `../bake-license-rust`.

Tasks are synchronous in this initial implementation. An individual task can start a runtime or a subprocess; Bake imposes no async runtime dependency.

## Releasing

Prepare a release with `cargo bake cargo:version:patch` (or `minor`, `major`, or `bump --version X.Y.Z`), then run `cargo bake cargo:release` and open a pull request. After review and merge, GitHub Actions publishes the release when the configured `crates-io` environment approves it. Follow the shared [Releasing skill](https://github.com/socketry/socketry-project-rust/blob/main/context/releasing.md) for the standard process.

## Releases

<!-- bake-readme:releases:start -->

See [releases.md](releases.md) for the full release history.

### v0.19.1

- Keep generated dependency skills out of the tracked repository.

- Adopt `socketry-project` 0.3.7 for shared project tasks and Markdown normalization.

- Require the aggregate test and coverage result for pull request merges.

- Refresh dependency examples and repository-owned agent guidance.

### v0.19.0

- Support unbounded positional `Vec<T>` task arguments with `#[bake(positional)]`. Require `::` before chaining another task.

### v0.18.0

- Derive default task namespaces from `bake_*` library crate names, stripping `bake_` and translating remaining underscores to colons. Preserve explicit names, project binary names, and matching module prefixes. Existing unnamed library tasks outside those prefixes gain a namespace; use explicit names to retain their previous commands.
- Use `bake = "0"` for reusable task libraries so linked crates resolve one Bake 0.x version and share its task registry.
- Resolve task names at compile time. Generated descriptors now include their full namespaces, making manual registration and automatic discovery agree. Manual registries that added those namespaces with `Registry::include` should register the descriptors directly to avoid repeating the prefix.
- Document semantic task-library APIs, crate-derived namespaces, and testing conventions aligned with `socketry-project`.

<!-- bake-readme:releases:end -->

## Contributing

Please open an issue or pull request on [GitHub](https://github.com/socketry/bake-rust).

### Agent Context

Run `cargo bake agent:context:install` to install shared context and skills. Read `.agents/context/index.md` to find relevant guides, follow `agents.md` if present, and apply skills under `.agents/skills/`. The installer preserves repository-owned `agents.md`; it does not create or regenerate that file.
