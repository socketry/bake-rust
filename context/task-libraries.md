# Structuring Bake Tasks in Crates

Bake has two common places for task functions:

- A project's unpublished `bake/` binary crate holds tasks specific to that
  project. The `cargo-bake` launcher compiles and runs it.
- A normal library crate can export reusable tasks alongside its Rust API. Other
  Bake binaries discover those tasks when they depend on and link that library.

## Keep project tasks in a small binary crate

For project-specific automation, put the task functions in the task binary or
its child modules. The crate can stay unpublished and separate from the project's
released libraries:

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

The command creates `bake/`, adds it to the workspace, and generates a minimal
binary. Later runs refresh its generated task-library links while preserving
the project's task source.

`main.rs` can declare modules and start discovery:

```rust,ignore
mod release;

fn main() -> bake::Result<()> {
    bake::Registry::discover()?.run()
}
```

Task functions in `release.rs` are ordinary Rust functions annotated with
`#[bake::task]`. Keep them next to the project automation they implement. If a
task module grows, split it into child modules and use their names to organize
the command list.

## Export tasks from a reusable library

Put reusable tasks in the library crate that owns the behavior they automate.
Use public, semantic modules rather than a generic `tasks` module, because module
names become command namespaces:

```text
socketry-executor/
├── Cargo.toml
└── src/
    ├── lib.rs
    └── executor.rs
```

```rust,ignore
// src/lib.rs
pub mod executor;
```

```rust,ignore
// src/executor.rs
use bake::{Context, Result};

/// Print information about the executor configuration.
#[bake::task]
pub fn print_name(context: &mut Context) -> Result<String> {
    Ok(format!("executor root: {}", context.root().display()))
}
```

The task is discovered as `executor:print_name`. Namespace parts come from the
Rust module path beneath the library crate root; package names do not add a
namespace. Nested modules add more parts. Underscores in module names become
hyphens. An explicit task name containing `:` is used as-is, without a module
prefix.

The same crate can export normal Rust APIs and Bake tasks. A task function is
still callable as an ordinary function; the attribute also generates its task
descriptor and registration entry. It needs a dependency on `bake` so it can
use the attribute and Bake types.

## Consume task libraries

Add a reusable task library as a dependency of the local task binary, then run
`cargo bake --regenerate` so its registration entries are linked:

```toml
[dependencies]
bake = "0.17"
socketry_executor = { package = "socketry-executor", version = "0.1" }
bake_agent_context = "0.1"
```

The first `cargo bake --regenerate` creates the private `bake/` workspace member
and its minimal binary if they do not exist. Each run regenerates a small source
file that links unconditional, non-optional, platform-independent direct
dependencies in `bake/Cargo.toml` (apart from `bake` itself), and adds a module
declaration to the selected binary if needed. It preserves the rest of the task
source. Put reusable task libraries in `[dependencies]`; ordinary dependencies
used by task code are also linked.

No per-task imports or registration calls are needed. The dependency's module
names define its task namespaces, and `Registry::discover()` reports an error
if two linked crates register the same task name. A crate used only as a normal
Rust dependency does not need to expose a Bake executable; Cargo does not run a
dependency's binary target when building your application.

The `bake-agent-context` task library adds `agent:context:list`,
`agent:context:show`, `agent:context:install`, and `agent:context:agents-md` to
the same executable. Run `cargo bake agent:context:install` to copy context from
resolved dependencies into `.agents/context/` and update `agents.md`. Ignore
`.agents/context/` because it is generated from the resolved dependencies.

If ordinary users of a library should not inherit its Bake dependency, put the
tasks in a separate companion crate, such as `socketry-executor-bake`. The
project's task binary then depends on and references that companion crate instead.
This keeps the main library's dependencies smaller, at the cost of one extra
package dependency for projects that want its tasks.

For a working example, see [Bake Releases](https://github.com/socketry/bake-releases-rust), whose
`releases` module provides `releases:notes` and `releases:update`. The [Bake
Cargo](https://github.com/socketry/bake-cargo-rust) crate adds Cargo workspace
discovery, package publishing, GitHub release creation, publish-workflow
generation, GitHub ruleset and environment setup, crates.io trusted-publisher
configuration, shared Cargo version changes, and release-candidate checks. Its
version tasks optionally call a project-defined `cargo:after_version_bump` task
with the new version. A private `bake/` crate can use that hook to compose
[Bake License](https://github.com/socketry/bake-license-rust), Bake Releases,
and other project-specific release tasks. If the hook is not registered, the
version task simply completes without project-specific updates.
