# Bake task macros

Procedural macros for `bake`. Most users access the attribute through
`#[bake::task]`, re-exported by the core library.

The attribute preserves the original synchronous function and generates a sibling
`function_task()` returning a task descriptor. It also adds the descriptor to Bake's
link-time registration table for `Registry::discover()`. For unnamed library
tasks, a Rust crate name beginning with `bake_` supplies a default namespace:
remove the prefix and replace remaining underscores with colons. Nested modules
extend that namespace; matching module prefixes are included only once. Cargo
binary targets keep their module-based names.

The macro emits constant-evaluated name construction using the defining
`module_path!()`. The generated descriptor already contains the final name;
manual registration and discovery use the same name. Discovery validates task
metadata and detects collisions across linked crates.

Use `name = "namespace:task"` to override the entire command name. A short
explicit name uses only the defining module's namespace, preserving existing
aliases and root commands. Use `runtime = ::alias` if the core dependency was
renamed from bake.

Use `#[bake::task(output)]` when a task handles its own user-facing output. Use
`#[bake(input)] input: bake::Value` to receive the result from the preceding task.

Tasks accept owned FromStr arguments and an optional context reference. They return
a Result with a serializable successful value and a displayable error. The macro
rejects async, unsafe, foreign, generic, variadic, and receiver-based signatures.

See the [workspace documentation](https://github.com/socketry/bake-rust) for examples.
