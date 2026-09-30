# Releases

## Unreleased

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
