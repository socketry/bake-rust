---
name: socketry-project-setup
description: Set up a new or existing Rust repository for a Socketry project, including its Cargo layout, shared Bake tasks, agent context, tests, and GitHub workflows. Use when creating a crate repository or bringing one into the standard project structure.
---

# Set Up a Rust Repository

Use this skill to start a Rust repository with the shared Socketry conventions,
agent context, and release tasks.

## Create the repository

Use a separate repository for a crate that needs its own version, release notes,
or release tag. Use a Cargo workspace when its packages are meant to ship
together at one version and share one `releases.md` and one `vVERSION` tag.

Choose the crate name for its public purpose. Crates.io has a flat package
namespace, so use a `socketry-` prefix when needed to identify a Socketry crate.
The repository can use the `-rust` suffix to distinguish it from a related
project in another language.

Start with the standard Cargo layout and root files described in the
Rust Repository Layout context guide provided by `socketry-project`. Keep the
library's dependencies in the root package and development automation in a
private `bake/` workspace member.

## Add shared project tasks

Install the Cargo launcher and bootstrap the private task package:

```sh
cargo install socketry-cargo-bake --locked
cargo bake --regenerate
```

The command creates `bake/`, adds it to the Cargo workspace, and writes a
minimal binary. Add this dependency under the existing `[dependencies]` table in
`bake/Cargo.toml`:

```toml
socketry-project = "0.2"
```

Run regeneration again to link its task registrations:

```sh
cargo bake --regenerate
```

Set release reviewers in the root `Cargo.toml`:

```toml
[workspace.metadata.bake.release]
reviewers = ["socketry/managers"]
```

The `socketry-project` dependency makes the shared tasks available to the
private Bake binary. It also registers `cargo:after_version_bump`, which updates
`license.md`, `releases.md`, and generated sections in `readme.md` after a
version change.
It bundles the standard `test` and `test:external` task providers as well.
Keep task tooling out of unrelated published libraries. Consumer projects
should depend on `socketry-project` from their private `bake/` package.

Install the project's agent context:

```sh
cargo bake agent:context:install
```

This installs ordinary context guides under `.agents/context/`, installs
dependency-provided skills under `.agents/skills/`, and updates `agents.md`
with links to ordinary context. Read that index and inspect applicable skills.
Add `--package socketry-project` to install only this crate's context and
skills. See the Agent Context guide provided by `bake-agent-context` for how to
organize shared and project-only guidance.

## Set up GitHub

Configure repository metadata, collaboration features, pull request defaults,
and branch protection using the `socketry-project-github-repository` skill.
Generate the Cargo workflow with `cargo:setup:workflow`; follow the
[`socketry-project-releasing` skill](https://github.com/socketry/socketry-project-rust/blob/main/context/releasing.md)
before applying rulesets, environment reviewers, or crates.io trusted
publishing.

## Standard workflows

Keep `.github/workflows/test.yml` for local workspace tests. When
`bake-test-rust` is linked, install the Cargo launcher and run
`cargo bake --locked test` so the optional `test:before` hook also runs.
Otherwise use `cargo test --workspace --locked`. Add platform or feature
matrix entries when the project needs them.

List selected downstream projects under
`[workspace.metadata.bake.test.external]` in the root `Cargo.toml`. Add
`.github/workflows/external.yml` only when that list is non-empty, and run
`cargo bake --locked test:external` when `bake-test-rust` is linked. The task
keeps checkouts under `external/` and applies local workspace crates as Cargo
patches so downstream tests exercise the source being developed.

Use `cargo:setup:workflow` from `bake-cargo` to generate
`.github/workflows/publish.yml`. That workflow checks a release candidate on
pull requests, publishes after merge through the configured `crates-io`
environment, and then creates or updates the matching GitHub Release from
`releases.md`. See the [`socketry-project-releasing` skill](https://github.com/socketry/socketry-project-rust/blob/main/context/releasing.md)
for trusted publishing and repository setup. See the Rust Testing context
guide provided by `socketry-project` for test workflow details and optional
downstream compatibility workflows.

## Work on the project

Keep the root `readme.md` concise and human-focused. Use the project context and
Rust API documentation for detailed implementation guidance. Run the project's
checks before opening a pull request, and update `releases.md` for user-visible
changes.
