# Development Context

This guide summarizes the implementation and task composition in the Bake
repository. It complements the public usage guides in this directory.

## Purpose

Bake is a Cargo-compatible task runner inspired by Samuel Williams's Ruby Bake.
The project-local task binary contains ordinary typed Rust functions. The launcher
discovers and runs that binary through Cargo. `#[bake::task]` registers functions
for `Registry::discover()`. Nested Rust modules define namespaces; task library
dependencies must be referenced by the executable to make the linker include them.

## Source map

- src/arguments.rs: parameter metadata, typed validation, command-line parsing.
- src/task.rs: task descriptor and handler interface.
- src/registry.rs: registration, namespaces, command planning, help and output.
- src/output.rs: replaceable default output, raw/JSON/NDJSON formatting, and null sink.
- src/context.rs: shared state, project root, previous result, nested calls.
- crates/macros/: task attribute and generated adapters; re-exported by bake.
- crates/cargo-bake/: Cargo discovery and process launcher; independent of the core.
- [bake-releases-rust](https://github.com/socketry/bake-releases-rust):
  release-document parsing and reusable notes/update tasks.
- [bake-cargo-rust](https://github.com/socketry/bake-cargo-rust):
  Cargo workspace, version, GitHub release, and publishing tasks.
- [bake-license-rust](https://github.com/socketry/bake-license-rust):
  license documents and Rust source copyright maintenance.
- [bake-agent-context-rust](https://github.com/socketry/bake-agent-context-rust):
  dependency context discovery, inspection, and installation.
- bake/: this repository's task binary and examples of composition.

The task binary links reusable task libraries through dependencies declared in
`bake/Cargo.toml`; each library must be referenced by the executable so its
registered tasks are linked into the binary.

## Important boundaries

- Task registration uses linkme's linker inventory. It is static, not a dynamic
  plugin ABI. Dependencies that contribute tasks must be referenced in the task
  binary, e.g. `use bake_releases as _;`.
- The launcher reads Cargo metadata format 1; it does not link Cargo internals.
- Package-level configuration takes precedence over workspace-level configuration.
- The core is synchronous. An async runtime can be owned by an individual task.
- A chain shares one Context. Nested calls use full registered names and update
  previous() on success. Calls are limited to 64 nested invocations.
- The registry includes `output` and `null`. It invokes `output` after the last task
  unless that task handles output. Reusable namespace imports skip these built-ins.
- `#[bake(input)] value: bake::Value` injects the previous result. The default
  output task returns that result after writing it, so later tasks can reuse it.
- Supplied values are validated before execution, then parsed by generated adapters.
  Default expressions run at invocation time.
- Release documents use unindented ATX headings and fenced code blocks. This is
  a deliberate narrow document format, not a complete CommonMark parser.
- The release updater replaces the document through a temporary file in the same
  directory, preserving file permissions. It follows an existing symlink to its target.

## Useful commands

Install this checkout's `socketry-cargo-bake` launcher before running the Bake
commands below:

```sh
cargo install --path crates/cargo-bake --locked
```

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo bake --locked --list
cargo bake --locked test:external
cargo bake --locked greet Samuel --excited true
cargo bake --locked releases:notes Unreleased
```

The private task binary links `bake-test-rust`, which registers `test` and
`test:external`. Selected downstream Bake projects are listed in the root
`Cargo.toml` under `[workspace.metadata.bake.test.external]`. The external task
checks those projects against the local workspace crates and keeps their
checkouts under the ignored `external/` directory. The External Tests workflow
runs the same task on pushes and pull requests.

Follow the current session's instructions about adding or running tests. Use
--offline with a populated Cargo cache when network access is unavailable.

The initial implementation and tests were developed on macOS. The GitHub workflow
also runs on Linux and Windows. Inspect the actual workflow results before claiming
verification on another platform.

For release preparation and registry setup, see the shared
[Cargo publishing guide](https://github.com/socketry/bake-cargo-rust/blob/main/context/publishing.md).
Publishing must be explicitly requested; ordinary development commands do not
release anything.
