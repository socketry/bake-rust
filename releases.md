# Releases

## Unreleased

- Derive default task namespaces from `bake_*` library crate names, stripping
  `bake_` and translating remaining underscores to colons. Preserve explicit
  names, project binary names, and matching module prefixes. Existing unnamed
  library tasks outside those prefixes gain a namespace; use explicit names to
  retain their previous commands.
- Resolve task names at compile time. Generated descriptors now include their
  full namespaces, making manual registration and automatic discovery agree.
  Manual registries that added those namespaces with `Registry::include` should
  register the descriptors directly to avoid repeating the prefix.
- Document semantic task-library APIs, crate-derived namespaces, and testing
  conventions aligned with `socketry-project`.

## v0.17.4

- Use the shared `socketry-project` Releasing skill for the standard release
  process and remove references to the duplicate Bake Cargo publishing context.

## v0.17.3

- Align the Readme's contribution guidance with Bake Readme conventions.
- Remove the local Bake alias and install the launcher explicitly for repository
  tasks.
- Require complete line coverage in CI with the standard Bake coverage task.

## v0.17.2

- Add `cargo bake --regenerate` to create and synchronize a project's private task crate.
- Integrate project release, license, agent-context, Readme, and external-test tasks.
- Add external tests for six downstream Socketry projects.

## v0.17.1

- Create or update GitHub Releases after successful crates.io publication.
- Resolve the local task crate during version updates.

## v0.17.0

- Publish the core library under the `bake` package name.
- Publish task authoring and development guides under `context/`.
- Add Bake Agent Context tasks to the project task executable.
- Move repository-only conventions and release instructions under `.agents/`.

## v0.2.1

- Add optional namespaced task hooks for composing project-specific automation.

## v0.2.0

- Separate Cargo release tasks into `bake-cargo` and add reusable Rust license maintenance in `bake-license`.
- Publish from reviewed release changes and create version tags after successful crate uploads.
- Document release candidate checks and the required crates.io environment approval.

## v0.1.0

- Add a Cargo launcher for compiled project-local task crates.
- Generate task descriptors, typed arguments, defaults, and help from Rust functions.
- Support explicit namespaces, reusable task libraries, chained results, and shared context.
- Add release-note extraction and updates for releases.md documents.
- Establish Rust conventions, agent context, and cross-platform continuous integration.
- Add GitHub Actions publishing to crates.io through trusted publishing.
