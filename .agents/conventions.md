# Bake Repository Conventions

- Keep shared Rust conventions in the `bake-agent-context` package. This file records Bake-specific choices.
- Use semantic Rust modules to define reusable task namespaces. Link task libraries once from the private `bake/` executable with `use crate_name as _;`.
- Keep project automation in the unpublished `bake/` workspace member. Never publish that package.
- Keep the core synchronous; task functions may start a runtime or subprocess when needed.
- Preserve the built-in `output` task behavior. Mark tasks that emit their own final output with `#[bake::task(output)]`.
- Keep the package workspace on one version and release history. Use sibling path plus version dependencies for locally developed packages.
- Keep public task authoring guidance in `context/` and internal implementation notes in `.agents/`.
