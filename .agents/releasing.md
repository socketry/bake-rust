# Releasing

All publishable packages share one version and one release tag. Creating a
GitHub repository or pushing main does not publish crates.

## Preparation

1. Run one of `cargo:version:patch`, `cargo:version:minor`,
   `cargo:version:major`, or `cargo:version:bump --version VERSION` to update
   the workspace packages together.
2. The version task invokes this project's `cargo:after_version_bump` hook with
   the new version. The hook runs `license:update` and renames the Unreleased
   heading in `releases.md` to `vVERSION`. Review those changes and the release
   notes.
3. Run the workspace tests and the formatting/Clippy checks.
4. Commit the reviewed release changes.

The version tasks update Cargo manifests and version requirements for
in-workspace dependencies, refresh `Cargo.lock`, and then invoke the optional
`cargo:after_version_bump` task if the project defines it. This repository's
hook updates its license and release notes. The version tasks do not commit the
changes.

## Crate name and trusted publishing

The crates.io `bake` name has historical yanked versions through 0.16.0, so
this workspace began at 0.17.0. The workspace's first release has been
published, and the `v0.17.0` tag is in place. Its GitHub trusted publisher is
configured for `socketry/bake-rust`, `publish.yml`, and the `crates-io`
environment.

For a new crate, register a trusted publisher after its first upload. Use the
repository, workflow, and environment that will publish it:

- Owner: socketry
- Repository: bake-rust
- Workflow: publish.yml
- Environment: crates-io

Configure the required environment reviewer in the workspace Cargo metadata;
the setup task applies it to GitHub's `crates-io` environment:

```toml
[workspace.metadata.bake.release]
reviewers = ["socketry/managers"]
```

Then review the setup plan and apply it:

```sh
cargo bake cargo:setup:github:plan --branch main
cargo bake cargo:setup:github:apply --branch main
```

The setup task resolves `socketry/managers` through the authenticated GitHub CLI.
An explicit `--reviewers` argument overrides Cargo metadata. The workflow uses
the Rust project's [crates.io authentication action](https://github.com/rust-lang/crates-io-auth-action)
to obtain a short-lived publishing token. Repository creation does not configure
the registry's trusted publishers automatically. GitHub administrators can
bypass environment reviewers by default; disable administrator bypass in the
environment settings if approval must also be mandatory for administrators.

The release-task libraries are maintained in their own repositories and have
independent versions and `releases.md` files. Their private `bake/` task
packages use published Bake and companion crates; sibling checkouts are not
needed. See the
[Bake Releases](https://github.com/socketry/bake-releases-rust) and
[Bake Cargo](https://github.com/socketry/bake-cargo-rust) repositories for their
release instructions. Cargo version tasks depend on
[Bake License](https://github.com/socketry/bake-license-rust), which is also
published independently. Release Bake Releases and Bake License before
`bake-cargo`, which depends on both.

For later releases, prepare the candidate and commit the version, release-note
changes, and other release edits together in a pull request:

```sh
cargo bake cargo:release
```

The task checks that the release notes contain the matching version heading and
that each publishable package can be packaged. It does not tag or publish. After
the pull request is reviewed and merged, the GitHub workflow detects the version
change, waits for approval from the `crates-io` environment reviewers, and
publishes every publishable package from that commit. It skips package versions
already on crates.io so a failed partial publication can be resumed safely, and
fails if the registry cannot be checked. Once all uploads succeed, the workflow
creates and pushes the annotated `vVERSION` tag. Tag pushes do not trigger
publication.

Do not reuse a tag or crates.io version; both are immutable release identifiers.

The workflow does not create a GitHub Release automatically. After the tag has
published the crates, create a GitHub Release from the matching notes with:

```sh
cargo bake releases:github:release vVERSION
```

The final command uses the GitHub CLI and publishes the release immediately. Add
`--draft true` to create a draft for review first.
