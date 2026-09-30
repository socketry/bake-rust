# Releasing

The workspace starts at version 0.1.0. Packages share a version, but are published
individually. Creating a GitHub repository or pushing main does not publish a crate.

## Preparation

1. Update workspace.package.version and the versions of local package dependencies.
2. Run cargo test --workspace --locked and the formatting/Clippy checks.
3. Rename the Unreleased heading with cargo bake releases:update vVERSION. Review
   the notes with cargo bake releases:notes vVERSION. Add a fresh Unreleased section
   for subsequent development.
4. Commit the reviewed release changes.

The release tasks currently manage notes only; manifest version changes remain
explicit edits.

## First publication

From a machine authenticated with crates.io, publish the packages in this order:

```sh
cargo publish --package socketry-bake-macros --locked
cargo publish --package socketry-bake --locked
cargo publish --package socketry-bake-releases --locked
cargo publish --package socketry-cargo-bake --locked
```

Wait for each dependency to become available before publishing its consumers.
The launcher has no dependency on the other Bake packages and can be published
independently. The bake-rust-tasks package is unpublished.

Package names are candidates until the registry accepts them; this repository
does not reserve names by itself. Review the package contents with cargo package
--list --package NAME before publication.

## GitHub publishing

After the packages exist, configure a crates.io trusted publisher for each package:

- Owner: socketry
- Repository: bake-rust
- Workflow: publish.yml
- Environment: crates-io

Configure the repository's crates-io environment as desired. The workflow uses
the Rust project's [crates.io authentication action](https://github.com/rust-lang/crates-io-auth-action)
to obtain a short-lived publishing token. Repository creation does not configure
the registry's trusted publishers automatically.

Push a tag of the form PACKAGE-vVERSION to publish that package:

```sh
git tag socketry-bake-macros-v0.1.0
git push origin socketry-bake-macros-v0.1.0
```

The workflow checks that the tag matches the selected manifest version, runs the
workspace tests, and publishes the package. Publish dependency tags first and wait
for success before tagging dependents. Do not push a publication tag for a version
that is already published; crates.io versions are immutable.

The workflow does not create a GitHub Release automatically. Release notes can be
extracted for one using cargo bake releases:notes vVERSION.
