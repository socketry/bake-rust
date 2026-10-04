# Conventions

- Keep shared Rust conventions in the `bake-agent-context` package. This file records Bake-specific choices.
- Follow `socketry-project` for crate/module paths and test layout. Use semantic Rust APIs and explicit names for exported namespaced tasks as described in `context/task-libraries.md`; follow `context/testing-task-libraries.md` for library, task, and executable tests.
- Add task libraries to the private `bake/` package and run `cargo bake --regenerate` to link them. Resolve the library under development to the current checkout, including transitive dependencies.
- Keep project automation in the unpublished `bake/` workspace member. Never publish that package.
- Keep the core synchronous; task functions may start a runtime or subprocess when needed.
- Preserve the built-in `output` task behavior. Mark tasks that emit their own final output with `#[bake::task(output)]`.
- Keep the package workspace on one version and release history. Use sibling path plus version dependencies for locally developed packages.
- Keep public task authoring guidance in `context/` and internal implementation notes in `.agents/`.
