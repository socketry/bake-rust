# Agent

## Context

This section links to documentation from installed packages. It is automatically generated and can be refreshed with `cargo bake agent:context:install`.

**Before working on a package, read the relevant context files below. They contain package-specific guidance and workflows.**

If these files are missing or dependencies have changed, run `cargo bake agent:context:install` to install them.

### bake

Composable, typed development tasks for Rust projects

#### [Design](.agents/context/bake/design.md)

The cargo-bake executable asks Cargo for workspace metadata, identifies an
unpublished task binary, and runs it.

#### [Development Context](.agents/context/bake/development.md)

This guide summarizes the implementation and task composition in the Bake
repository.

#### [Structuring Bake Tasks in Crates](.agents/context/bake/task-libraries.md)

Bake has two common places for task functions:

### bake-agent-context

Install context files from Cargo dependencies for coding agents

#### [Getting Started](.agents/context/bake-agent-context/getting-started.md)

This guide shows how to add Bake Agent Context tasks to a Rust project and install context from its Cargo dependencies.

#### [Using and Providing Context](.agents/context/bake-agent-context/usage.md)

Use Bake Agent Context to discover and install practical guidance shipped in dependency crates.

#### [Agent Context](.agents/context/bake-agent-context/agent-context.md)

Keep reusable package guidance separate from repository-only instructions,
and make both easy for agents to discover.

#### [Rust agent context](.agents/context/bake-agent-context/rust.md)

Use this shared guidance when working on Socketry's Rust crates.

#### [Agent Context Specification](.agents/context/bake-agent-context/specification.md)

Agent Context is a language-agnostic specification for providing and consuming contextual information from software packages to assist AI agents and other automated tools.

### bake-cargo

Cargo project and release automation tasks for Bake

#### [Releasing](.agents/skills/socketry-project-releasing/SKILL.md)

Follow the shared release process for Socketry Rust projects.

### bake-readme

Reusable readme.md maintenance tasks for Bake

#### [Readme Structure](.agents/context/bake-readme/readme-structure.md)

Use this guidance when creating or updating a project's readme.md.

### bake-test-rust

Reusable Rust test tasks for Bake

#### [Rust Testing Tasks](.agents/context/bake-test-rust/testing.md)

Add bake-test-rust as a dependency of the private bake/ package and link it
from bake/src/main.rs:

### socketry-markdown

CommonMark compliant markdown parser in Rust with ASTs and extensions

#### [Markdown Parser Architecture](.agents/context/socketry-markdown/markdown-architecture.md)

This crate implements a Markdown state machine that tokenizes source, resolves
constructs, and can either compile events directly to HTML or build an mdast
syntax tree.

### socketry-project

Shared project conventions and development tasks for Socketry Rust crates

#### [Socketry Rust Conventions](.agents/context/socketry-project/conventions.md)

Use these conventions for Rust repositories in the Socketry organization.

#### [Rust Repository Layout](.agents/context/socketry-project/layout.md)

Use Cargo's standard structure so contributors can find package code, tests,
examples, and project documentation quickly.

#### [Rust Testing](.agents/context/socketry-project/testing.md)

Use Cargo's built-in test command for Rust tests.
