# Conventions

## Files and packages

- Use lowercase Markdown filenames, including readme.md, license.md, and releases.md.
- Start every license.md with `# MIT License`. Preserve upstream attribution.
- Follow Cargo's src/, tests/, and examples/ layout.
- Use the socketry- prefix for published package names; use semantic Rust names
  such as bake and bake_releases in source code.
- Keep development tasks in the unpublished bake/ workspace member.
- Commit Cargo.lock so the launcher and development tasks have reproducible dependencies.

## Rust code

- Avoid abbreviations in source code. Prefer clear, consistent naming. Keep names
  required by standard traits and upstream APIs.
- Use cargo fmt formatting and safe Rust. Expected failures return Result with
  an actionable message; reserve unwrap and expect for tests or proven invariants.
- Keep registration explicit. Reusable task libraries use ordinary Cargo dependencies.
- Use concrete types and established traits. Keep runtime requirements out of
  the synchronous core.
- Pass task context explicitly. Do not change the process-wide working directory
  or environment to execute tasks.
- Pass subprocess arguments as separate values. Use a shell only when the task
  explicitly requires shell semantics.
- Validate a command chain before executing it. Stop execution on failure.
- Keep package versions in workspace.package and use path plus version dependencies
  for publishable workspace packages.

## Documentation and verification

- Describe implemented behavior and limitations separately from future ideas.
- Document defaults, namespace behavior, filesystem writes, and process execution.
- Put public API tests in tests/. Private procedural macro expansion tests can
  live in an implementation test module.
- Use isolated temporary projects for launcher and file-modifying tests. Never
  publish, tag, or mutate a user's checkout from a test.
- Run the requested verification and report exactly what passed on which platform.
- Keep agent-context.md and readme examples synchronized with the public API.
