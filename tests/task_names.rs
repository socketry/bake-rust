// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

mod build_tools {
    #[bake::task]
    pub fn check_all() -> bake::Result<()> {
        Ok(())
    }

    #[bake::task(name = "show")]
    pub fn inspect() -> bake::Result<()> {
        Ok(())
    }
}

#[test]
fn manual_registration_uses_module_namespaces_outside_bake_libraries() {
    let discovered = bake::Registry::discover().unwrap();
    let mut manual = bake::Registry::new();
    for (descriptor, expected) in [
        (build_tools::check_all_task(), "build-tools:check_all"),
        (build_tools::inspect_task(), "build-tools:show"),
    ] {
        assert_eq!(descriptor.name(), expected);
        assert!(discovered.tasks().any(|task| task.name() == expected));
        manual.register(descriptor).unwrap();
    }
    let mut context = manual.context(".");
    assert!(
        context
            .call("build-tools:check_all", &[])
            .unwrap()
            .is_null()
    );
    assert!(context.call("build-tools:show", &[]).unwrap().is_null());
    assert!(context.call("check_all", &[]).is_err());
}
