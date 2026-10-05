# Testing Task Libraries

Verify a task library's semantic Rust API, registered Bake interface, and executable composition at their respective boundaries.

Follow [Structuring Bake Tasks in Crates](task-libraries.md) for implementation and command naming. Socketry's [layout guide](https://github.com/socketry/socketry-project-rust/blob/main/context/layout.md) defines where unit and integration tests belong, and its [testing guidance](https://github.com/socketry/socketry-project-rust/blob/main/context/testing.md) defines coverage expectations. Use the `bake-test-rust` context for the shared test commands and CI workflows.

## Test semantic operations

Exercise parsing, transformations, discovery, and updates through the ordinary Rust interface. Use public API integration tests for caller-visible behavior and module-local tests where private implementation details need verification. Keep substantial implementation suites with the library that owns them.

Cover meaningful failure behavior as well as successful results. For file updates, this can include preservation of unrelated content, repeated updates, and the state left after a failed write. For publishing or subprocess tasks, check which operations happened before a failure and whether later operations were skipped. Test these properties where the implementation owns them.

## Test registered task contracts

Calling an annotated Rust function directly does not exercise discovery or generated argument conversion. Add integration tests in the library's `tests/` directory that link the library, discover its tasks, and invoke them by their public command names. For example, a release-task test can use:

```rust,ignore
use bake_releases as _;

#[test]
fn reads_release_notes_from_the_context_root() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("releases.md"),
        "## v1.0.0\nNew feature.\n",
    )
    .unwrap();

    let registry = bake::Registry::discover().unwrap();
    let mut context = registry.context(directory.path());
    let result = context.call("releases:notes", &["v1.0.0"]).unwrap();

    assert_eq!(result.as_str(), Some("New feature.\n"));
}
```

Check each library's registered names and relevant defaults, required and optional arguments, project-root behavior, result values, and propagated errors. Select representative cases for each adapter; keep the full domain case matrix with the operation tests. Bake itself owns exhaustive tests of its generic argument parser and formatter.

If a library exposes generated descriptors for manual registration, check that their names and invocation behavior agree with automatic discovery. Descriptors already contain the complete name resolved during compilation.

For hooks, use a fresh `Registry` and context state to record calls and their arguments. Assert invocation order, optional-hook absence, and error propagation where these are part of the contract. Register or replace small recording handlers instead of running unrelated release or publishing operations. Keep tests of a shared hook's full behavior in the crate that provides that hook.

## Test executable composition

Keep a small suite under `bake/tests/` that launches the private executable using `CARGO_BIN_EXE_<binary-name>`. Verify task discovery through `--list`, plus representative command execution, stdout/stderr, and success/failure exit status. A registry-level test complements these checks but does not exercise the process entry point.

Ensure these tests link the task library from the current checkout. Check the resolved dependency graph for a registry copy of the package under development, especially when `socketry-project` supplies tasks transitively. Testing a published copy does not verify local changes to registration or adapters.

Keep the private binary small. Its tests establish that the selected libraries compose correctly; the reusable libraries own their detailed behavior tests.

## Keep fixtures isolated and purposeful

Use `tempfile::TempDir` for owned temporary projects and RAII cleanup. Share fixture builders within a test suite when they express useful operations such as creating a Cargo workspace or committing a Git revision. Prefer these established facilities over custom timestamp-based temporary directories.

Configure working directories and environment variables on child `Command` instances. For in-process operation tests, pass executable paths, clients, or other collaborators explicitly where the operation needs them. Avoid mutating the test process's environment to select a fake tool. A mutex used by some tests does not isolate those changes from other code running in the same process.

Use local Git repositories, fake executables, and local HTTP servers when the behavior crosses those boundaries. Record arguments and control results so tests can inspect success, failure, and partial completion without contacting live publishing services. Gate platform-specific fixtures explicitly and retain portable tests for the shared contract.

Introduce failure injection at a narrow boundary where real error handling needs verification. Keep ordinary and coverage builds behaviorally aligned; avoid exposing public coverage-only APIs or reproducing a library's private failure tests in its development executable just to satisfy instrumentation. Review both the uncovered behavior and the coverage configuration when results differ between builds.

Start with suite-local support modules. Extract shared fixture code across crates when the repeated behavior has a clear common contract; each library can retain fixtures specific to its domain.
