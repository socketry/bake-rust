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
descriptor and registration entry. It needs a dependency on `socketry-bake` so
it can use the attribute and Bake types.

## Consume task libraries

Add a reusable task library as a dependency of the local task binary, then
reference it once so its registration entries are linked:

```toml
[dependencies]
bake = { package = "socketry-bake", version = "0.1" }
socketry_executor = { package = "socketry-executor", version = "0.1" }
```

```rust,ignore
// src/main.rs
use socketry_executor as _;

fn main() -> bake::Result<()> {
    bake::Registry::discover()?.run()
}
```

No per-task imports or registration calls are needed. The dependency's module
names define its task namespaces, and `Registry::discover()` reports an error
if two linked crates register the same task name. A crate used only as a normal
Rust dependency does not need to expose a Bake executable; Cargo does not run a
dependency's binary target when building your application.

If ordinary users of a library should not inherit its Bake dependency, put the
tasks in a separate companion crate, such as `socketry-executor-bake`. The
project's task binary then depends on and references that companion crate instead.
This keeps the main library's dependencies smaller, at the cost of one extra
package dependency for projects that want its tasks.

For a working example, see [Bake Releases](crates/releases/readme.md), whose
`releases` module provides the `releases:notes` and `releases:update` tasks.
