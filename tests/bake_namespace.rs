// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use bake::{Registry, Result};

#[bake::task]
fn inspect() -> Result<String> {
    Ok("root operation".to_owned())
}

mod version {
    #[bake::task]
    pub fn bump(version: String) -> bake::Result<String> {
        Ok(version)
    }

    #[bake::task(name = "show")]
    pub fn inspect() -> bake::Result<String> {
        Ok("explicit short name".to_owned())
    }

    #[bake::task(name = "release:prepare")]
    pub fn prepare() -> bake::Result<String> {
        Ok("explicit full name".to_owned())
    }
}

#[bake::task(name = "test")]
fn run_tests() -> Result<String> {
    Ok("explicit root name".to_owned())
}

pub use version::bump;

mod namespace {
    #[bake::task]
    pub fn legacy() -> bake::Result<String> {
        Ok("matching module prefix".to_owned())
    }
}

#[test]
fn descriptors_and_discovery_use_the_same_final_names() {
    let discovered = Registry::discover().unwrap();
    let mut manual = Registry::new();
    for (descriptor, expected) in [
        (inspect_task(), "namespace:inspect"),
        (version::bump_task(), "namespace:version:bump"),
        (version::inspect_task(), "version:show"),
        (version::prepare_task(), "release:prepare"),
        (run_tests_task(), "test"),
        (namespace::legacy_task(), "namespace:legacy"),
    ] {
        assert_eq!(descriptor.name(), expected);
        assert!(discovered.tasks().any(|task| task.name() == expected));
        manual.register(descriptor).unwrap();
    }
    let mut context = manual.context(".");
    assert_eq!(
        context.call("namespace:version:bump", &["2.0.0"]).unwrap(),
        "2.0.0"
    );
    assert_eq!(
        context.call("namespace:legacy", &[]).unwrap(),
        "matching module prefix"
    );
}

#[test]
fn discovers_and_invokes_defaults_from_the_defining_crate_and_modules() {
    // Cargo compiles this target as `bake_namespace`, exercising the macro's
    // actual module_path!() registration with a Bake library-style crate name.
    let mut context = Registry::discover().unwrap().context(".");
    assert_eq!(
        context.call("namespace:inspect", &[]).unwrap(),
        "root operation"
    );
    assert_eq!(
        context.call("namespace:version:bump", &["1.2.3"]).unwrap(),
        "1.2.3"
    );
    assert_eq!(bump("2.0.0".to_owned()).unwrap(), "2.0.0");
    assert!(context.call("namespace:bump", &["1.2.3"]).is_err());
}

#[test]
fn preserves_explicit_task_names() {
    let mut context = Registry::discover().unwrap().context(".");
    assert_eq!(context.call("test", &[]).unwrap(), "explicit root name");
    assert_eq!(
        context.call("version:show", &[]).unwrap(),
        "explicit short name"
    );
    assert_eq!(
        context.call("release:prepare", &[]).unwrap(),
        "explicit full name"
    );
}
