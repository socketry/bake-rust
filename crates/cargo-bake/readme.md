# Cargo Bake

The cargo-bake binary compiles and runs a project's unpublished bake/ task crate.
Install from a checkout with `cargo install --path crates/cargo-bake --locked`.

```sh
cargo bake --help
cargo bake --list
cargo bake releases:notes Unreleased
```

The launcher discovers the nearest project manifest and uses Cargo metadata to
find its workspace. The default task manifest is workspace-root/bake/Cargo.toml.
Configure workspace.metadata.bake.manifest, or package.metadata.bake.manifest for
a package-specific override. An override path is relative to the corresponding root.

Use --manifest-path to select a project, --offline to disable Cargo network access,
--locked to require unchanged lockfiles, and --release to use the release profile.
Place launcher flags before task arguments. --list and TASK --help are handled by
the compiled task binary; --help explains the launcher without compiling it.

Task crates can use any compatible binary implementation; the launcher only sets
BAKE_PROJECT_ROOT and forwards arguments. Commands execute without a shell and
child exit codes are preserved.

See the [workspace documentation](https://github.com/socketry/bake-rust) for setup.
