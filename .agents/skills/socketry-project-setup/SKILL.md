---
name: socketry-project-setup
description: Bootstrap a Rust repository with Socketry layout, Bake tasks, agent context, tests, and workflows. Use for a new repository or initial setup.
---

# Set Up a Rust Repository

Use this skill to bootstrap a new Rust repository or add its initial shared
Socketry tooling. For an audit of an established repository, use the
`socketry-project-update` skill.

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
socketry-project = "0.3"
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
Keep task tooling out of unrelated published libraries. Consumer projects
should depend on `socketry-project` from their private `bake/` package.

## Agent context

Follow the [Agent Context section in `readme.md`](../readme.md#agent-context)
to install and discover shared context and skills. Follow
[Conventions](conventions.md#source-and-documentation) for where to keep
package guidance and project-only instructions. The `bake-agent-context`
guide documents installer behavior and options.

## Set up GitHub

Configure repository metadata, collaboration features, pull request defaults,
and branch protection using the `socketry-project-github-repository` skill.
Generate the Cargo workflow with `cargo:setup:workflow`; follow the
`socketry-project-releasing` skill before applying rulesets, environment
reviewers, or crates.io trusted publishing. It links to the Bake Cargo Readme
for task-specific details.

## Test workflows

Use the `socketry-project-testing` skill for organization-wide testing
expectations. Consult the installed `bake-test-rust` context for canonical
`test.yml` and optional `external.yml` workflows, task setup, coverage options,
and downstream test configuration.

Use `cargo:setup:workflow` from `bake-cargo` to generate
`.github/workflows/publish.yml`. That workflow checks a release candidate on
pull requests, publishes after merge through the configured `crates-io`
environment, and then creates or updates the matching GitHub Release from
`releases.md`. Follow the `socketry-project-releasing` skill for the release
process and `bake-test-rust` context for workflow and task details.

## Work on the project

Keep the root `readme.md` concise and human-focused. Use the project context and
Rust API documentation for detailed implementation guidance. Run the project's
checks before opening a pull request, and update `releases.md` for user-visible
changes.
