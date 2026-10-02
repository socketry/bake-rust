---
name: socketry-project-pull-requests
description: Prepare commits and GitHub pull requests for Socketry Rust projects. Use when writing commit messages, creating or updating a pull request, choosing its issue type, or deciding what to include in the description and release notes.
---

# Pull Requests

Use these conventions when preparing commits or pull requests for Socketry Rust
projects.

## Titles and commits

* Pull request titles must use Markdown, be complete sentences, and end with a
  full stop.
* Commit messages must use Markdown and end with a full stop.
* The first line of a commit message must focus on what changed.
* Most commit messages should be a single line.
* Keep relevant context in the code itself, such as comments, rather than using
  the commit message as a side channel for important details.
* Do not include agent links, attribution footers, generated-by annotations, or
  similar metadata in commit messages.

## Pull request description

Start with a brief summary, followed by a detailed description of the problem
and solution. Include implementation details that help reviewers understand
the change, link relevant issues when applicable, and include screenshots for
visual changes.

Use this structure, replacing the guidance with project-specific content:

```markdown
Briefly summarize the change in 1–3 sentences.

Describe the problem, context, and solution. Include implementation details
that help reviewers understand the change. Link relevant issues if applicable.
Include screenshots for visual changes.
```

Do not add a `Types of Changes` section. Use GitHub issue type metadata for
classification instead.

## Testing

Changes should include suitable test coverage. Aim for complete coverage of the
behavior being changed or introduced. If a change directly affects downstream
crates, add or update downstream integration coverage when useful.

Do not list passing test commands or verification steps in the pull request
description unless they explain an unusual risk, limitation, or manual
validation requirement.

## Release notes

For user-visible changes, add a brief entry to `releases.md` following the
release notes guidance provided by `bake-releases`.

## Issue type

Set the GitHub issue type correctly when creating or updating a pull request:

* Use `Bug` for defect fixes and regressions.
* Use `Feature` for new user-facing capabilities.
* Use `Task` for maintenance, refactoring, documentation, tests, release work,
  and internal improvements.
