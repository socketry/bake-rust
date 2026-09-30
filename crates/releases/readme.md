# Bake Releases

Reusable releases.md tasks for Bake, inspired by Samuel Williams's
[Ruby bake-releases](https://github.com/ioquatix/bake-releases) (MIT).

The task functions live under the `releases` module, which becomes the task
namespace. Add this crate as a dependency and reference it from the task binary
so Rust links its registration entries:

```rust,ignore
use bake_releases as _;

bake::Registry::discover()?.run()
```

No per-task registration calls are needed. `#[bake::task]` functions in the
library are collected by `Registry::discover()`.

```sh
cargo bake releases:notes Unreleased
cargo bake releases:update v0.1.0
cargo bake releases:notes v0.1.0 --path releases.md
```

## Document format

Use unindented ATX headings such as `## Unreleased` and `## v0.1.0`. The version
argument matches the complete heading title exactly (including any v prefix).
Optional closing heading markers are supported. Fenced code blocks can contain
example headings. Setext headings, HTML blocks, and headings inside lists or
block quotes are outside this release-document format.

notes returns the body beneath the selected heading until the next heading of
the same or a higher level. Nested sections, whitespace, line endings, and other
Markdown bytes are preserved. Missing and duplicate headings are errors.

update renames exactly one Unreleased heading. It rejects an existing release
heading, missing/duplicate Unreleased sections, and invalid multiline titles.
The rest of the file is preserved. A temporary file in the same directory is
written and synchronized before replacing the original, preserving file permissions.
Existing symlinks are followed. As with other file editors, concurrent external
edits are not merged and replacing a file changes its identity for hard links.

These tasks do not modify Cargo versions, create a new Unreleased section, commit,
tag, or publish. The pure extract_notes and update_document functions are also
available for direct library use.
