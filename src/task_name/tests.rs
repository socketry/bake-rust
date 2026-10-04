// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use super::TaskName;

// Evaluate each case at compile time, as the macro does, and at runtime so
// coverage can observe the same name construction and normalization paths.
macro_rules! resolved {
    ($name:expr, $module:expr, $infer:expr) => {{
        const STORAGE: TaskName<{ $module.len() + $name.len() + 1 }> =
            TaskName::new($name, $module, $infer);
        const NAME: &str = match str::from_utf8(STORAGE.as_bytes()) {
            Ok(name) => name,
            Err(_) => panic!("task name must remain valid UTF-8"),
        };
        // Share a runtime instantiation so coverage combines every case.
        let runtime = TaskName::<1024>::new($name, $module, $infer);
        assert_eq!(str::from_utf8(runtime.as_bytes()).unwrap(), NAME);
        NAME
    }};
}

#[test]
fn resolves_library_names_and_complete_matching_module_prefixes() {
    for (actual, expected) in [
        (
            resolved!("inspect", "bake_releases", true),
            "releases:inspect",
        ),
        (
            resolved!("inspect", "bake_agent_context", true),
            "agent:context:inspect",
        ),
        (
            resolved!("bump", "bake_cargo::version", true),
            "cargo:version:bump",
        ),
        (
            resolved!("check_all", "bake_cargo::build_tools", true),
            "cargo:build-tools:check_all",
        ),
        (
            resolved!("bump", "bake_example::example::version", true),
            "example:version:bump",
        ),
        (
            resolved!("list", "bake_agent_context::context", true),
            "agent:context:context:list",
        ),
        (
            resolved!("list", "bake_agent_context::context::skill", true),
            "agent:context:context:skill:list",
        ),
        (
            resolved!("list", "bake_example::agent::other", true),
            "example:agent:other:list",
        ),
        (
            resolved!("list", "bake_example::agent", true),
            "example:agent:list",
        ),
        (
            resolved!("list", "bake_example::agent::contextual", true),
            "example:agent:contextual:list",
        ),
        (
            resolved!("list", "bake_agent_context::agent_context", true),
            "agent:context:agent-context:list",
        ),
        (
            resolved!("inspect", "bake_cargo::cargo_tools", true),
            "cargo:cargo-tools:inspect",
        ),
        (
            resolved!("inspect", "bake_cargo::carg", true),
            "cargo:carg:inspect",
        ),
        (
            resolved!("inspect", "bake_example::example", true),
            "example:inspect",
        ),
        (
            resolved!("inspect", "bake_cargo::releases::github", true),
            "cargo:releases:github:inspect",
        ),
    ] {
        assert_eq!(actual, expected);
    }
}

#[test]
fn preserves_explicit_names_binaries_and_other_crates() {
    for (actual, expected) in [
        (
            resolved!("project:inspect", "bake_example::other_module", true),
            "project:inspect",
        ),
        (
            resolved!("agent:context:list", "bake_agent_context::context", true),
            "agent:context:list",
        ),
        (resolved!("test", "bake_test_rust", false), "test"),
        (
            resolved!("package", "bake_example::cargo", false),
            "cargo:package",
        ),
        (resolved!("greet", "bake_rust_tasks", false), "greet"),
        (
            resolved!("check", "bake_rust_tasks::build_tools", false),
            "build-tools:check",
        ),
        (
            resolved!("inspect", "crate::my_module::build_tasks", true),
            "my-module:build-tasks:inspect",
        ),
        (resolved!("inspect", "crate", true), "inspect"),
        (resolved!("inspect", "bake_", true), "inspect"),
        (
            resolved!("inspect", "bake_::nested", true),
            "nested:inspect",
        ),
        (resolved!("inspect", "bake", true), "inspect"),
        (resolved!("inspect", "socketry_project", true), "inspect"),
        (
            resolved!("inspect", "socketry_project::releases", true),
            "releases:inspect",
        ),
        (resolved!("inspect", "releases_bake", true), "inspect"),
        (resolved!("", "bake_example", true), "example:"),
        (
            resolved!("inspect", "bake_café::résumé", true),
            "café:résumé:inspect",
        ),
        (resolved!("résumé", "bake_example", true), "example:résumé"),
    ] {
        assert_eq!(actual, expected);
    }
}
