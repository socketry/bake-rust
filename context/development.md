# Development Context

This guide summarizes the implementation and task composition in the Bake repository. It complements the public usage guides in this directory.

## Purpose

Bake is a Cargo-compatible task runner inspired by Samuel Williams's Ruby Bake. The project-local task binary contains ordinary typed Rust functions. The launcher discovers and runs that binary through Cargo. `#[bake::task]` registers functions for `Registry::discover()`. Library crates named `bake_*` supply default task prefixes, extended by nested Rust modules. Project binaries keep module-based naming. Task library dependencies must be referenced by the executable to make the linker include them.

## Source map

- src/arguments.rs: parameter metadata, typed validation, command-line parsing.
- src/task.rs: task descriptor and handler interface.
- src/task\_name.rs: constant-evaluated crate and module namespace inference.
- src/registry.rs: registration, namespace imports, command planning, help and output.
- src/output.rs: replaceable default output, raw/JSON/NDJSON formatting, and null sink.
- src/context.rs: shared state, project root, previous result, nested calls.
- crates/macros/: task attribute and generated adapters; re-exported by bake.
- crates/cargo-bake/: Cargo discovery and process launcher; independent of the core.
- [bake-releases-rust](https://github.com/socketry/bake-releases-rust): release-document parsing and reusable notes/update tasks.
- [bake-cargo-rust](https://github.com/socketry/bake-cargo-rust): Cargo workspace, version, GitHub release, and publishing tasks.
- [bake-license-rust](https://github.com/socketry/bake-license-rust): license documents and Rust source copyright maintenance.
- [bake-agent-context-rust](https://github.com/socketry/bake-agent-context-rust): dependency context discovery, inspection, and installation.
- bake/: this repository's task binary and examples of composition.

The task binary links reusable task libraries through dependencies declared in `bake/Cargo.toml`; each library must be referenced by the executable so its registered tasks are linked into the binary.

## Important boundaries

- Task registration uses linkme's linker inventory. It is static, not a dynamic plugin ABI. Dependencies that contribute tasks must be referenced in the task binary, e.g. `use bake_releases as _;`.
- The macro resolves names through Rust constant evaluation. Generated descriptors have their final names before discovery; the registry validates and collects them.
- The launcher reads Cargo metadata format 1; it does not link Cargo internals.
- Package-level configuration takes precedence over workspace-level configuration.
- The core is synchronous. An async runtime can be owned by an individual task.
- A chain shares one Context. Nested calls use full registered names and update previous() on success. Calls are limited to 64 nested invocations.
- The registry includes `output` and `null`. It invokes `output` after the last task unless that task handles output. Reusable namespace imports skip these built-ins.
- `#[bake(input)] value: bake::Value` injects the previous result. The default output task returns that result after writing it, so later tasks can reuse it.
- Supplied values are validated before execution, then parsed by generated adapters. Default expressions run at invocation time.
- Required scalars are positional; `Option` and `Vec` are named by default. The opt-in positional `Vec` is unbounded and ends only at `::` or the end of the invocation, so it must be the last positional argument.
- Release documents use unindented ATX headings and fenced code blocks. This is a deliberate narrow document format, not a complete CommonMark parser.
- The release updater replaces the document through a temporary file in the same directory, preserving file permissions. It follows an existing symlink to its target.

## Useful commands

Install `socketry-cargo-bake` before running the Bake commands below:

```sh
cargo install socketry-cargo-bake --locked
```

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo bake --locked test --all-targets true
cargo bake --locked test:coverage --all-targets true
cargo bake --locked --list
cargo bake --locked test:external
cargo bake --locked greet Samuel --excited true
cargo bake --locked releases:notes Unreleased
```

The private task binary links `bake-test-rust`, which registers `test` and `test:external`. Selected downstream Bake projects are listed in the root `Cargo.toml` under `[workspace.metadata.bake.test.external]`. The external task checks those projects against the local workspace crates and keeps their checkouts under the ignored `external/` directory. The External Tests workflow runs the same task on pushes and pull requests.

The Test workflow runs `cargo bake --locked test` on macOS and the coverage task on Ubuntu. Windows runs `cargo test --workspace --locked` directly because the task runner is part of this workspace and Windows cannot replace its executable while it is running. The Ubuntu job also runs formatting and Clippy. Coverage runs workspace and documentation tests, invokes the optional `test:before` hook, and requires 100% line coverage. The External Tests workflow is separate because this workspace lists selected downstream projects in `[workspace.metadata.bake.test.external]`.

Follow the current session's instructions about adding or running tests. Use --offline with a populated Cargo cache when network access is unavailable.

Inspect actual workflow results before claiming verification on another platform.

For release preparation and registry setup, follow the shared [Releasing skill](https://github.com/socketry/socketry-project-rust/blob/main/context/releasing.md). Publishing must be explicitly requested; ordinary development commands do not release anything.
