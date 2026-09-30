# Agent context

Read conventions.md before changing this repository.

## Purpose

Bake is a Cargo-compatible task runner inspired by Samuel Williams's Ruby Bake.
The project-local task binary contains ordinary typed Rust functions. The launcher
discovers and runs that binary through Cargo. Reusable libraries register functions
under project-selected namespaces.

## Source map

- src/arguments.rs: parameter metadata, typed validation, command-line parsing.
- src/task.rs: task descriptor and handler interface.
- src/registry.rs: registration, namespaces, command planning, help and output.
- src/context.rs: shared state, project root, previous result, nested calls.
- crates/macros/: task attribute and generated adapters; re-exported by bake.
- crates/cargo-bake/: Cargo discovery and process launcher; independent of the core.
- crates/releases/: release-document parsing and reusable notes/update tasks.
- bake/: this repository's task binary and examples of composition.

## Important boundaries

- Task registration is explicit; there is no linker inventory or dynamic plugin ABI.
- The launcher reads Cargo metadata format 1; it does not link Cargo internals.
- Package-level configuration takes precedence over workspace-level configuration.
- The core is synchronous. An async runtime can be owned by an individual task.
- A chain shares one Context. Nested calls use full registered names and update
  previous() on success. Calls are limited to 64 nested invocations.
- Supplied values are validated before execution, then parsed by generated adapters.
  Default expressions run at invocation time.
- Release documents use unindented ATX headings and fenced code blocks. This is
  a deliberate narrow document format, not a complete CommonMark parser.
- The release updater replaces the document through a temporary file in the same
  directory, preserving file permissions. It follows an existing symlink to its target.

## Useful commands

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo bake --locked --list
cargo bake --locked greet Samuel --excited
cargo bake --locked releases:notes Unreleased
```

Follow the current session's instructions about adding or running tests. Use
--offline with a populated Cargo cache when network access is unavailable.

The initial implementation and tests were developed on macOS. The GitHub workflow
also runs on Linux and Windows. Inspect the actual workflow results before claiming
verification on another platform.

For publication order and required registry setup, see releasing.md. Publishing
must be explicitly requested; ordinary development commands do not release anything.
