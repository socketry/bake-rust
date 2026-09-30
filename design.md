# Design

## Cargo owns compilation

The cargo-bake executable asks Cargo for workspace metadata, identifies an
unpublished task binary, and runs it. The task binary depends on the core and any
reusable task libraries through normal Cargo dependencies. Cargo handles builds,
dependency resolution, and compiled artifact reuse.

This follows the practical direction of Aaron Turon's
[2018 workflow proposal](https://aturon.github.io/tech/2018/04/05/workflows/)
and the [xtask pattern](https://github.com/matklad/cargo-xtask). It uses Cargo's
existing custom commands and metadata; a native Cargo tasks table is unnecessary.

## Functions define interfaces

The task attribute generates a descriptor and argument adapter next to the original
function. The function remains directly callable. Required scalars are positional;
defaults, Option, and Vec make named arguments. Function documentation becomes
task help. The macro checks unsupported signatures and lets Rust check argument
traits, output serialization, and the original function body.

Descriptors are explicitly registered. Libraries return registries, and consumers
choose namespaces when importing them. This provides compile-time dependencies
and deterministic runtime names without global discovery.

## Execution is sequential and contextual

Planning validates all command names and supplied argument values before any task
handler starts. Successful results become the context's previous value. Context
also holds project-local state and builds subprocess commands with a local working
directory. Hooks are ordinary function calls or explicit nested task invocations.

Only the last result is formatted automatically. Tasks decide how to report
progress and diagnostics. A task error stops the chain and includes its task name.
Panics retain ordinary Rust behavior and are not treated as recoverable task errors.

## Scope

The initial release provides synchronous tasks, explicit registration, typed
command-line values, reusable libraries, help, structured output, composition,
and release-note tasks. It does not automatically discover functions across crates
or infer a dependency graph. Async execution, richer parsers, and additional release
automation can be added when a concrete task needs them.

No Ruby implementation files are vendored. The release task behavior is inspired
by Samuel Williams's MIT-licensed bake-releases; the Rust implementation is original.
