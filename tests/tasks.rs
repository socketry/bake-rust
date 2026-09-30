use bake::{Context, Error, Parameter, Registry, Result, Task, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

#[bake::task]
fn greet(
    name: String,
    #[bake(default = false)] excited: bool,
    #[bake(default = 1)] repeat: usize,
) -> Result<String> {
    Ok(format!("{name}{}", if excited { "!" } else { "." }).repeat(repeat))
}

#[bake::task]
fn options(
    output: Option<PathBuf>,
    labels: Vec<String>,
    #[bake(default = "releases.md")] release_path: PathBuf,
) -> Result<Value> {
    bake::value((output, labels, release_path))
}

#[bake::task]
fn add(left: i64, right: i64) -> Result<i64> {
    Ok(left + right)
}

#[bake::task]
fn previous(context: &mut Context) -> Result<Value> {
    Ok(context.previous().clone())
}

#[bake::task]
fn remember(context: &mut Context, value: usize) -> Result<()> {
    context.insert(value);
    Ok(())
}

#[bake::task]
fn increment(#[bake(context)] execution: &mut Context) -> Result<usize> {
    let value = execution
        .get_mut::<usize>()
        .ok_or_else(|| Error::new("no remembered value"))?;
    *value += 1;
    Ok(*value)
}

#[bake::task]
fn nested(context: &mut Context) -> Result<Value> {
    context.call("add", &["10", "20"])?;
    context.call("previous", &[])
}

#[bake::task]
fn recurse(context: &mut Context) -> Result<Value> {
    context.call("recurse", &[])
}

#[bake::task]
fn required(#[bake(named, help = "A required named argument.")] message: String) -> Result<String> {
    Ok(message)
}

#[bake::task]
fn fail() -> Result<()> {
    Err(Error::new("intentional failure"))
}

static CALLS: AtomicUsize = AtomicUsize::new(0);
#[bake::task]
fn effect() -> Result<()> {
    CALLS.fetch_add(1, Ordering::SeqCst);
    Ok(())
}

#[bake::task]
fn r#type(__bake_arguments: String, __bake_context: String) -> Result<String> {
    Ok(format!("{__bake_arguments}:{__bake_context}"))
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    for task in [
        greet_task(),
        options_task(),
        add_task(),
        previous_task(),
        remember_task(),
        increment_task(),
        nested_task(),
        recurse_task(),
        required_task(),
        fail_task(),
        effect_task(),
        type_task(),
    ] {
        registry.register(task).unwrap();
    }
    registry
}

fn run(arguments: &[&str]) -> Result<String> {
    registry().run_arguments(
        ".",
        &arguments
            .iter()
            .map(|argument| (*argument).to_owned())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn positional_arguments_and_typed_defaults() {
    assert_eq!(run(&["greet", "Samuel"]).unwrap(), "Samuel.\n");
    assert_eq!(
        run(&["greet", "Samuel", "repeat=2", "--excited"]).unwrap(),
        "Samuel!Samuel!\n"
    );
    assert_eq!(greet("Sam".into(), false, 1).unwrap(), "Sam.");
}

#[test]
fn named_argument_spellings_and_boolean_values() {
    for arguments in [
        vec!["greet", "Sam", "excited=true"],
        vec!["greet", "--name=Sam", "--excited=true"],
        vec!["greet", "--excited", "true", "Sam"],
        vec!["greet", "name=Sam", "--excited"],
    ] {
        assert_eq!(run(&arguments).unwrap(), "Sam!\n");
    }
    assert_eq!(
        run(&["greet", "Sam", "--excited", "false"]).unwrap(),
        "Sam.\n"
    );
    assert_eq!(run(&["greet", "Sam", "--excited=false"]).unwrap(), "Sam.\n");
}

#[test]
fn optional_and_repeated_values_and_hyphenated_names() {
    assert_eq!(
        serde_json::from_str::<Value>(&run(&["options"]).unwrap()).unwrap(),
        serde_json::json!([null, [], "releases.md"])
    );
    let output = run(&[
        "options",
        "--output",
        "some file",
        "labels=one",
        "--labels=two",
        "--release-path",
        "changes.md",
    ])
    .unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        serde_json::json!(["some file", ["one", "two"], "changes.md"])
    );
}

#[test]
fn negative_numbers_and_literal_positional_arguments() {
    assert_eq!(run(&["add", "-2", "5"]).unwrap(), "3\n");
    assert_eq!(
        run(&["greet", "--", "--name=value"]).unwrap(),
        "--name=value.\n"
    );
    assert_eq!(run(&["greet", "--name=--flag"]).unwrap(), "--flag.\n");
}

#[test]
fn argument_errors_are_actionable() {
    for (arguments, message) in [
        (vec!["greet"], "missing argument"),
        (vec!["greet", "Sam", "repeat=many"], "invalid usize"),
        (vec!["greet", "Sam", "--unknown"], "unknown argument"),
        (vec!["greet", "Sam", "--name", "Again"], "more than once"),
        (vec!["greet", "Sam", "--repeat"], "requires a value"),
        (
            vec!["greet", "Sam", "--repeat", "::", "previous"],
            "requires a value",
        ),
        (vec!["required"], "missing argument"),
        (vec!["missing"], "unknown task"),
        (vec!["::"], "expected a task name"),
        (vec!["previous", "::"], "expected a task after"),
    ] {
        assert!(
            run(&arguments).unwrap_err().to_string().contains(message),
            "{arguments:?}"
        );
    }
}

#[test]
fn required_named_arguments() {
    assert_eq!(run(&["required", "--message", "Hello"]).unwrap(), "Hello\n");
}

#[test]
fn chain_validation_precedes_all_effects() {
    CALLS.store(0, Ordering::SeqCst);
    assert!(run(&["effect", "::", "greet", "Sam", "repeat=invalid"]).is_err());
    assert!(run(&["effect", "::", "missing"]).is_err());
    assert_eq!(CALLS.load(Ordering::SeqCst), 0);
}

#[test]
fn implicit_and_explicit_chains_pass_previous_results() {
    assert_eq!(run(&["add", "2", "3", "previous"]).unwrap(), "5\n");
    assert_eq!(run(&["add", "2", "3", "::", "previous"]).unwrap(), "5\n");
    assert_eq!(
        run(&["greet", "Sam", "--excited", "previous"]).unwrap(),
        "Sam!\n"
    );
}

#[test]
fn task_state_is_shared_within_a_context() {
    assert_eq!(
        run(&["remember", "40", "::", "increment", "::", "increment"]).unwrap(),
        "42\n"
    );
    assert!(run(&["increment"]).is_err());
}

#[test]
fn nested_tasks_and_recursion_limit() {
    assert_eq!(run(&["nested"]).unwrap(), "30\n");
    assert!(
        run(&["recurse"])
            .unwrap_err()
            .to_string()
            .contains("depth exceeded")
    );
    let mut context = registry().context(".");
    assert!(context.call("previous", &["previous"]).is_err());
    assert!(context.call("fail", &[]).is_err());
    assert_eq!(context.call("add", &["2", "3"]).unwrap(), 5);
}

#[test]
fn failure_stops_a_chain() {
    let mut context = registry().context(".");
    context.call("remember", &["5"]).unwrap();
    assert!(
        context
            .call("fail", &[])
            .unwrap_err()
            .to_string()
            .starts_with("fail:")
    );
    assert_eq!(*context.get::<usize>().unwrap(), 5);
    assert!(run(&["fail", "::", "previous"]).is_err());
}

#[test]
fn json_and_silent_unit_output() {
    assert_eq!(run(&["--json", "greet", "Sam"]).unwrap(), "\"Sam.\"\n");
    assert_eq!(run(&["remember", "1"]).unwrap(), "");
    assert_eq!(run(&["--json", "remember", "1"]).unwrap(), "null\n");
}

#[test]
fn help_contains_parameters_and_is_sorted() {
    let help = run(&["greet", "--help"]).unwrap();
    assert!(help.contains("repeat: usize"));
    assert!(help.contains("default: 1"));
    assert!(
        run(&["required", "--help"])
            .unwrap()
            .contains("A required named argument.")
    );
    let listing = run(&["--list"]).unwrap();
    assert!(listing.find("add").unwrap() < listing.find("greet").unwrap());
    assert_eq!(run(&[]).unwrap(), listing);
}

#[test]
fn namespaces_and_collisions_are_explicit() {
    let mut registry = registry();
    assert!(registry.register(greet_task()).is_err());
    let mut library = Registry::new();
    library.register(greet_task()).unwrap();
    registry.include("examples", library).unwrap();
    let mut collision = Registry::new();
    collision
        .register(add_task())
        .unwrap()
        .register(greet_task())
        .unwrap();
    assert!(registry.include("examples", collision).is_err());
    assert!(!registry.tasks().any(|task| task.name() == "examples:add"));
    let mut context = registry.context(".");
    assert_eq!(context.call("examples:greet", &["Sam"]).unwrap(), "Sam.");
}

#[test]
fn descriptor_validation() {
    let handler = |_: &mut Context, _: &bake::Arguments| Ok(Value::Null);
    for name in ["", ":bad", "bad::name", "bad name", "--json"] {
        // Reserved launcher-style names must not be registerable as tasks.
        assert!(
            Registry::new()
                .register(Task::new(name, "", vec![], handler))
                .is_err(),
            "{name}"
        );
    }
    assert!(
        Registry::new()
            .register(Task::new(
                "valid",
                "",
                vec![
                    Parameter::new::<String>("same"),
                    Parameter::new::<String>("same")
                ],
                handler
            ))
            .is_err()
    );
}

#[test]
fn context_processes_have_a_local_working_directory() {
    let context = registry().context("project directory");
    assert_eq!(
        context.command("cargo").get_current_dir(),
        Some(std::path::Path::new("project directory"))
    );
}

#[test]
fn macro_identifiers_do_not_shadow_user_arguments() {
    assert_eq!(run(&["type", "one", "two"]).unwrap(), "one:two\n");
}
