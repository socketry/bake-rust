---
name: socketry-project-github-repository
description: Create or maintain GitHub repository metadata, collaboration features, merge settings, and main-branch protections for Socketry projects. Use when setting up a new repository or auditing its existing settings.
---

# GitHub Repository Setup

Use these defaults when creating a Socketry repository. Preserve deliberate
project-specific settings when maintaining an existing repository.

## Repository metadata

* Use the canonical Socketry organization and project name.
* Write a short, accurate repository description.
* Set the homepage to the documentation site when one exists.
* Add focused topics for discovery; avoid repeating words already in the
  repository name.
* Use `main` as the default branch.

## Collaboration features

Enable Issues, Discussions, Pull Requests, Sponsorships, and repository
preservation. Disable Projects and Wiki unless the project has a concrete use
for them.

Use GitHub issue types consistently:

* `Bug` for defects and regressions.
* `Feature` for new user-facing capabilities.
* `Task` for maintenance, refactoring, documentation, tests, and release work.

Prefer organization or repository labels that already exist. Add labels only
when the project needs a reusable classification not covered by issue types or
existing labels. Do not repeat issue type information in issue or pull request
text when GitHub already records it as metadata.

## Pull requests and commits

* Disable merge commits; allow squash and rebase merging.
* Suggest updating pull request branches, allow auto-merge, and delete merged
  head branches automatically.
* Require contributors to sign off on commits made through GitHub's web
  interface.
* Allow comments on individual commits.
* Use Markdown, complete sentences, and a final period for pull request titles
  and commit messages.
* Keep most commit messages to one line. Start with what changed.
* Describe pull requests with a short summary followed by the problem and
  solution. Use GitHub issue type metadata instead of adding a separate change
  type section.

Use the `socketry-project-pull-requests` skill for the full title, commit,
description, testing, and release note conventions.

## Branch protection

Protect `main` and require pull requests with at least one approval. Allow
administrators to bypass these rules for maintenance. Require only stable
checks that are needed for safe auto-merge; do not make experimental,
informational, or unreliable coverage checks mandatory.

## Apply settings safely

Use the GitHub CLI for repository changes. Inspect the target first and always
use its full name in commands:

```sh
gh repo view socketry/PROJECT
```

When maintaining an existing repository, inspect its current settings and make
the smallest change that achieves the intended result. Preserve project-specific
settings. Do not rename, archive, transfer, or delete a repository without
explicit approval, and do not disable issues, pull requests, or required checks
without approval.

The `socketry-project-releasing` skill describes the standard Cargo release
process and links to Bake Cargo task documentation for branch rulesets,
crates.io environment reviewers, and trusted publishing.
