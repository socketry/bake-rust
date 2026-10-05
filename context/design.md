# Design

## Cargo owns compilation

The cargo-bake executable asks Cargo for workspace metadata, identifies an unpublished task binary, and runs it. The task binary depends on the core and any reusable task libraries through normal Cargo dependencies. Cargo handles builds, dependency resolution, and compiled artifact reuse.

This follows the practical direction of Aaron Turon's [2018 workflow proposal](https://aturon.github.io/tech/2018/04/05/workflows/) and the [xtask pattern](https://github.com/matklad/cargo-xtask). It uses Cargo's existing custom commands and metadata; a native Cargo tasks table is unnecessary.

## Functions define interfaces

The task attribute generates a descriptor and argument adapter next to the original function. The function remains directly callable. Required scalars are positional; defaults, Option, and Vec make named arguments. A Vec can opt into unbounded positional arguments with `#[bake(positional)]`; it consumes bare values until `::` or the end of the task invocation. Function documentation becomes task help. The macro checks unsupported signatures and lets Rust check argument traits, output serialization, and the original function body.

Each task macro adds a descriptor to a linkme distributed slice. The executable uses `Registry::discover()` to collect tasks from itself and linked dependencies. Unnamed tasks in library crates named `bake_*` derive a prefix from the crate name, removing `bake_` and replacing remaining underscores with colons. Nested modules extend the namespace; an existing matching module prefix is included only once. Project binaries and explicit names retain module-based naming, and a fully qualified explicit name bypasses inference. The macro emits constant-evaluated name construction from `module_path!()` and the task attributes. Generated descriptors contain their final names, so manual registration and discovery agree. Discovery validates metadata and detects duplicate names across the linked libraries. Follow [Structuring Bake Tasks in Crates](task-libraries.md) for semantic APIs, task adapters, and compatibility conventions. Rust omits unused dependencies from the final link, so each task library must be referenced by the executable (an import such as `use bake_releases as _;` is enough). This avoids registration code for each task while keeping task libraries as ordinary Cargo dependencies.

## Execution is sequential and contextual

Planning validates all command names and supplied argument values before any task handler starts. Successful results become the context's previous value. Context also holds project-local state and builds subprocess commands with a local working directory. Hooks are ordinary function calls or explicit nested task invocations.

Only the last result is formatted automatically. Tasks decide how to report progress and diagnostics. A task error stops the chain and includes its task name. Panics retain ordinary Rust behavior and are not treated as recoverable task errors.

Automatic formatting goes through the registered `output` task. It receives the last task's result as an injected value. Explicit `output` commands therefore work in chains, write to stdout or a project-relative file, and return the original value for further processing. Projects can replace the registered output task. A task that already emits output can mark itself with `#[bake::task(output)]` to suppress the automatic formatter. The `null` task consumes a result without displaying it.

## Scope

The initial release provides synchronous tasks, link-time discovery, typed command-line values, reusable libraries, help, structured output, composition, and release-note tasks. It does not infer a dependency graph: task libraries must be Cargo dependencies and referenced by the executable to be linked. Async execution, richer parsers, and additional release automation can be added when a concrete task needs them.

No Ruby implementation files are vendored. The release task behavior is inspired by Samuel Williams's MIT-licensed bake-releases; the Rust implementation is original.
