# Releasing

The workspace starts at version 0.1.0. All publishable packages share one version
and one release tag. Creating a GitHub repository or pushing main does not publish crates.

## Preparation

1. Run `cargo bake releases:version:patch`, `minor`, `major`, or
   `bump --version VERSION` to update the workspace packages together.
2. Run the workspace tests and the formatting/Clippy checks.
3. Rename the Unreleased heading with `cargo bake releases:update vVERSION`. Review
   the notes with cargo bake releases:notes vVERSION. Add a fresh Unreleased section
   for subsequent development.
4. Commit the reviewed release changes.

The version tasks update Cargo manifests and local path dependency requirements.
They do not update the release notes or commit the changes.

## First publication

From a machine authenticated with crates.io, publish the workspace:

```sh
cargo publish --workspace --locked
```

Cargo selects the publishable workspace packages and skips the unpublished
`bake-rust-tasks` package.

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

The release-task libraries are maintained in their own repositories and have
independent versions and `releases.md` files. See the
[Bake Releases](https://github.com/socketry/bake-releases-rust) and
[Bake Cargo Releases](https://github.com/socketry/bake-releases-cargo-rust)
repositories for their release instructions.

For later releases, commit the reviewed version and release-note changes, then run:

```sh
cargo bake releases:cargo:release
```

The task packages the workspace, creates and pushes a `vVERSION` tag, and the
GitHub workflow publishes every publishable package from that commit. The
workflow verifies that all those packages have the tagged version, skips package
versions already on crates.io, and fails if the registry cannot be checked. Do
not reuse a tag or crates.io version; both are immutable release identifiers.

The workflow does not create a GitHub Release automatically. After the tag has
published the crates, create a GitHub Release from the matching notes with:

```sh
cargo bake releases:github:release vVERSION
```

The final command uses the GitHub CLI and publishes the release immediately. Add
`--draft true` to create a draft for review first.
