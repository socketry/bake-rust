# Structuring Bake Tasks in Crates

Define reusable automation as semantic Rust APIs and expose those operations as Bake tasks with stable command names.

For Socketry projects, follow the [crate and module conventions](https://github.com/socketry/socketry-project-rust/blob/main/context/conventions.md) and [source layout](https://github.com/socketry/socketry-project-rust/blob/main/context/layout.md) provided by `socketry-project`. This guide explains how those conventions apply to Bake task libraries. See [Testing Task Libraries](testing-task-libraries.md) for verification at the library, task, and executable boundaries.

Bake has two common places for task functions:

- A project's unpublished `bake/` binary crate holds tasks specific to that project. The `cargo-bake` launcher compiles and runs it.
- A normal library crate can export reusable tasks alongside its Rust API. Other Bake binaries discover those tasks when they depend on and link that library.

## Keep project tasks in a small binary crate

For project-specific automation, put the task functions in the task binary or its child modules. The crate can stay unpublished and separate from the project's released libraries:

```text
project/
├── Cargo.toml
├── src/
└── bake/
    ├── Cargo.toml
    └── src/
        ├── main.rs
        └── release.rs
```

Install the launcher once and bootstrap the task crate with:

```sh
cargo install socketry-cargo-bake --locked
cargo bake --regenerate
```

The command creates `bake/`, adds it to the workspace, and generates a minimal binary. Later runs refresh its generated task-library links while preserving the project's task source.

`main.rs` can declare modules and start discovery:

```rust,ignore
mod release;

fn main() -> bake::Result<()> {
    bake::Registry::discover()?.run()
}
```

Task functions in `release.rs` are ordinary Rust functions annotated with `#[bake::task]`. Keep them next to the project automation they implement. Split growing modules by domain, and preserve existing command names when moving functions between modules.

## Start with a semantic Rust interface

Put reusable tasks in the library crate that owns the behavior they automate. Give ordinary Rust callers an interface expressed in the library's domain:

- Use functions for operations such as `extract_notes(document, version)` or `update_document(document, version)`.
- Use objects when they own meaningful state, such as an `Installer` that holds discovered packages and supports `install_all` and `install_package`.
- Accept the paths, documents, options, and collaborators an operation needs. Keep project-root resolution and task invocation at the Bake boundary.
- Return domain values and errors that callers can inspect. Use descriptive result types when several related values form one result.

Standardize vocabulary, inputs, results, and failure semantics where libraries perform the same kind of operation. For example, `list` enumerates available items, `show` retrieves selected content, and `update` maintains an existing resource. Use `plan` and `apply` when inspecting proposed changes separately from applying them is useful. Add only the operations the domain needs.

Preserve domain-specific signatures. Add a shared trait when real consumers need interchangeable implementations. Use options structs for coherent sets of options and objects for meaningful state; small operations can remain functions.

Follow Socketry's rule that the crate name supplies the root Rust namespace. Re-export the main API from private implementation modules. Public modules should identify a meaningful part of the domain. Do not add a public wrapper such as `bake_releases::releases` solely to obtain a command prefix.

## Expose operations as Bake tasks

A task function adapts command arguments and execution context to the semantic API. Keep substantial parsing, transformations, and filesystem or subprocess operations in the implementation that ordinary Rust callers use. The adapter can resolve paths, select options, obtain context state, invoke hooks, and choose the task result.

When an operation already has a suitable task signature, annotate it directly. Separate adapters are useful when the library accepts borrowed values, owns state, or needs a different calling interface. Avoid forwarding layers that add no meaning. Task functions remain callable as ordinary Rust functions; their visibility should reflect the intended Rust API.

For example, this illustrative `bake_releases` library puts the task in `lib.rs` and keeps file reading and document parsing in private modules:

```text
src/
├── lib.rs
├── document.rs
└── notes.rs
```

```rust,ignore
// src/lib.rs
use bake::{Context, Result};
use std::path::PathBuf;

mod document;
mod notes;

pub use document::extract_notes;
pub use notes::read_notes;

/// Extract the notes for an exact release heading.
#[bake::task]
pub fn notes(
    context: &mut Context,
    version: String,
    #[bake(
        default = "releases.md",
        help = "Release document relative to the project root."
    )]
    path: PathBuf,
) -> Result<String> {
    read_notes(&context.root().join(path), &version)
}
```

```rust,ignore
// src/notes.rs
use bake::{Error, Result};
use std::path::Path;

/// Read the notes for an exact release heading from a document file.
pub fn read_notes(path: &Path, version: &str) -> Result<String> {
    let document = std::fs::read_to_string(path)
        .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    Ok(crate::extract_notes(&document, version)?.to_owned())
}
```

The parser in `document.rs` supplies `extract_notes<'a>(document: &'a str, version: &str) -> Result<&'a str>`. The example illustrates a proposed library layout; it does not describe the current exports of `bake-releases`. Rust callers can use the parser, call `read_notes` with an explicit path, or call the Bake adapter. They do not need to construct a `Context` to read a file.

## Let the library define the default task namespace

Use domain namespaces and meaningful operations, such as `releases:notes`, `license:update`, and `cargo:version:bump`. Existing task names and argument contracts are compatibility boundaries.

For a library crate whose Rust name starts with `bake_`, `#[bake::task]` derives a namespace by removing that prefix and replacing the remaining underscores with colons. It then appends nested modules and the function name:

| Task definition | Task name |
| --- | --- |
| `bake_releases::notes` with `#[bake::task]` | `releases:notes` |
| `bake_agent_context::install` with `#[bake::task]` | `agent:context:install` |
| `bake_agent_context::skill_list` with `name = "agent:context:skill:list"` | `agent:context:skill:list` |
| `bake_cargo::version::bump` with `#[bake::task]` | `cargo:version:bump` |

The crate supplies the Rust namespace and the default command prefix. An additional public `releases` module would repeat the domain already named by `bake_releases`; likewise, a public `context` module would repeat the final segment of `bake_agent_context`. Keep task placement close to the semantic operations it exposes. Use the default attribute when the defining crate, modules, and function already express the intended task name; use a full explicit name when a meaningful module path would otherwise add an unwanted command segment or the function name does not express the command's semantic form.

Underscores in module names retain their existing hyphen conversion; function names remain unchanged. The namespace uses the defining Rust crate name, even when a consumer renames its dependency. Re-exporting a function does not change its defining module path. For existing code whose modules already start with the complete crate-derived namespace, inference includes the prefix only once. New task libraries should avoid that redundant module layer; for example, define `notes` in `bake_releases` instead of `bake_releases::releases`.

Project binary targets keep their existing module-based naming, including binaries whose names start with `bake_`. Cargo identifies those targets through `CARGO_BIN_NAME`. Libraries without the `bake_` prefix also keep module-based naming; their crate name does not become a command prefix.

Use an explicit `name` when a command intentionally differs from the default or needs to remain stable across implementation moves. Explicit names retain their existing behavior: a name containing `:` is used as-is, while a short name receives only its defining module's namespace. Neither receives a new crate-derived prefix. Thus a root function marked `#[bake::task(name = "test")]` still exports `test` from `bake_test_rust`. A function marked `#[bake::task(name = "releases:notes")]` keeps that command from any module.

The attribute generates argument conversion, a descriptor, and registration. The library depends on `bake` to use these facilities. No additional registration framework or common task-library trait is needed.

Reusable task libraries should declare `bake = "0"` so Cargo can resolve their registry dependency to the Bake 0.x version selected by the consuming task binary. Project-local task binaries can select a specific minor release, such as `bake = "0.19"`.

Names are resolved during compilation from the defining `module_path!()`, the attribute, and Cargo's binary-target metadata. The generated `notes_task()` descriptor already contains `releases:notes`; registering it manually produces the same name as `Registry::discover()`. Discovery collects descriptors, validates their metadata, and detects collisions across linked crates.

## Keep task contracts predictable

- Use typed parameters. Required scalars are positional by default; defaults, `Option`, and `Vec` produce named options. Use `#[bake(named)]` for a required named option. A `Vec` can opt into positional variadic arguments with `#[bake(positional)]`; it must be the last positional parameter, and callers must use `::` before a following task. Document arguments with `help`.
- Resolve project-relative paths against `context.root()`. Use `context.command(...)` for project subprocesses, or pass an explicit working directory to an operation acting on another checkout. Avoid changing the process-wide working directory or environment.
- Validate inputs before making changes. Include the affected file, package, or command in errors when that context helps identify the failure. Propagate failures from operations and hooks.
- Return the semantic result. Text is appropriate for release notes; a list or serializable result struct suits package metadata or an update summary. Use `()` when there is no useful result. Avoid making callers parse a success message to recover counts, paths, or other structured data. Existing output shapes also need compatibility consideration when migrating a library.
- Let Bake's `output` task format the final result. Send progress and diagnostics to stderr. Use `#[bake::task(output)]` only when the task owns its final output; subprocess progress alone does not require it.

Use ordinary Rust calls to compose implementation operations. Use `context.call(...)` where composition is intentionally through registered tasks, including project-defined hooks. Use `call_if_registered` for optional hooks and document when they run and which arguments they receive. Libraries can cache project-scoped discovery in `Context` when several tasks need it; the underlying API should also accept the discovered state directly.

## Consume task libraries

Add a reusable task library as a dependency of the local task binary, then run `cargo bake --regenerate` so its registration entries are linked:

```sh
cargo bake --regenerate
cargo add --manifest-path bake/Cargo.toml socketry-executor --rename socketry_executor
cargo add --manifest-path bake/Cargo.toml bake-agent-context
cargo bake --regenerate
```

The first `cargo bake --regenerate` creates the private `bake/` workspace member and its minimal binary if they do not exist. Each run regenerates a small source file that links unconditional, non-optional, platform-independent direct dependencies in `bake/Cargo.toml` (apart from `bake` itself), and adds a module declaration to the selected binary if needed. It preserves the rest of the task source. Put reusable task libraries in `[dependencies]`; ordinary dependencies used by task code are also linked.

No per-task imports or registration calls are needed. The task attributes and defining modules determine command names. `Registry::discover()` reports an error if two linked crates register the same task name. A crate used only as a normal Rust dependency does not need to expose a Bake executable; Cargo does not run a dependency's binary target when building your application.

The `bake-agent-context` task library provides dependency context discovery and installation in the same executable. Run `cargo bake agent:context:install` to install guidance from resolved dependencies. Follow that package's context for the generated index, skills, and repository-owned agent instructions.

If ordinary users of a library should not inherit its Bake dependency, put the tasks in a separate companion crate, such as `socketry-executor-bake`. The project's task binary then depends on and references that companion crate instead. This keeps the main library's dependencies smaller, at the cost of one extra package dependency for projects that want its tasks.

## Use shared project composition

In Socketry repositories, depend on `socketry-project` in the private `bake/` package to obtain the standard development tasks and release hook. Its `cargo:after_version_bump` hook composes license, release-note, and readme updates. Keep this composition in the shared project crate, and keep the local executable focused on project-specific additions. A reusable task library should not depend on `socketry-project` merely to obtain its own development tooling.

During development of a task library, ensure the private executable resolves that library to the current checkout, including when it arrives transitively through `socketry-project`. For example, the root workspace manifest of `bake-releases` can contain:

```toml
[patch.crates-io]
bake-releases = { path = "." }
```

The local package version must satisfy the dependency requirements. Inspect Cargo's resolved dependency graph; adding a direct path dependency alone can leave a second registry copy linked through another dependency. Refresh the lockfile when resolution changes, and regenerate task links when dependencies of the private executable change.

## Align an existing library

Start by identifying the public Rust operations, registered command names, arguments, results, and hook behavior. Preserve those contracts while moving implementation into semantic modules and adding root re-exports. Keep existing Rust import paths as compatibility re-exports when needed, and document any intentional breaking API or output change.

Review unnamed tasks in `bake_*` libraries when adopting crate-derived namespaces. Tasks at the crate root gain a prefix, and tasks under a different domain gain the crate prefix as well. For example, the default name for `bake_cargo::releases::github::release` is `cargo:releases:github:release`. Call that full name in workflows and task chains; the short `releases:github:release` compatibility alias was removed in Bake Cargo 0.3.0. The standard publishing workflow also uses `cargo:release:detect`, `cargo:publish:pending`, and `cargo:release:publish` for the release lifecycle. Use explicit names to preserve existing command contracts where needed. Existing module paths that already start with the crate's domain are unchanged.

Generated descriptors now carry their complete names before registration, including module namespaces for libraries without a `bake_` prefix and for explicit short names. Review manual registries that previously added those namespaces using `Registry::include`: it still prepends the supplied namespace, so importing an already namespaced descriptor can repeat that prefix. Register the descriptor directly when its inferred name is the intended command.

Then align the task adapters, local dependency resolution, and tests with this guide. Reuse Socketry's layout and testing guidance, and record domain-specific exceptions in the library's own context. Apply the conventions as each library is reviewed; the illustrative APIs above do not imply that all current task libraries have already adopted them.
