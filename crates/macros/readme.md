# Bake task macros

Procedural macros for socketry-bake. Most users access the attribute through
`#[bake::task]`, re-exported by the core library.

The attribute preserves the original synchronous function and generates a sibling
`function_task()` returning a task descriptor. Register that descriptor explicitly.
Use `name = "namespace:task"` to override the command name, and `runtime = ::alias`
if the core dependency was renamed from bake.

Tasks accept owned FromStr arguments and an optional context reference. They return
a Result with a serializable successful value and a displayable error. The macro
rejects async, unsafe, foreign, generic, variadic, and receiver-based signatures.

See the [workspace documentation](https://github.com/socketry/bake-rust) for examples.
