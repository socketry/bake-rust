// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{Arguments, Context, Error, Format, Result, Task, output};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;

/// Task registry with deterministic listing and collision errors.
pub struct Registry {
    pub(crate) tasks: BTreeMap<String, Task>,
}

pub(crate) struct Invocation {
    pub(crate) name: String,
    pub(crate) arguments: Arguments,
}

/// A task descriptor registered by `#[bake::task]` in a linked crate.
#[doc(hidden)]
pub struct TaskRegistration {
    pub factory: fn() -> Task,
    pub builtin: bool,
}

#[linkme::distributed_slice]
pub static TASK_REGISTRATIONS: [TaskRegistration];

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.split(':').all(|part| {
            part.chars()
                .next()
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
                && part.chars().all(|character| {
                    character.is_ascii_alphanumeric() || character == '_' || character == '-'
                })
        })
}

impl Registry {
    pub fn new() -> Self {
        let mut registry = Self {
            tasks: BTreeMap::new(),
        };
        for task in output::builtins() {
            registry.tasks.insert(task.name.clone(), task);
        }
        registry
    }

    /// Discover task functions registered by `#[bake::task]` in this executable
    /// and its linked dependencies. Nested Rust modules become task namespaces.
    /// Library crates named `bake_*` also supply a default namespace, with the
    /// prefix removed and remaining underscores replaced by colons.
    /// Names are resolved at compile time in the generated descriptors, so
    /// manual registration uses the same names as discovery.
    ///
    /// A dependency that contributes tasks must be referenced by the executable
    /// (for example, `use bake_releases as _;`) so the linker includes it.
    pub fn discover() -> Result<Self> {
        Self::discover_from(&TASK_REGISTRATIONS)
    }

    fn discover_from(registrations: &[TaskRegistration]) -> Result<Self> {
        let mut registry = Self::new();
        for registration in registrations {
            if registration.builtin {
                continue;
            }
            registry.register((registration.factory)())?;
        }
        Ok(registry)
    }

    pub fn register(&mut self, task: Task) -> Result<&mut Self> {
        validate_task(&task)?;
        if self.tasks.contains_key(&task.name) {
            return Err(Error::new(format!("duplicate task {:?}", task.name)));
        }
        self.tasks.insert(task.name.clone(), task);
        Ok(self)
    }

    /// Replace an existing task explicitly, most often the default `output` task.
    pub fn replace(&mut self, name: &str, task: Task) -> Result<&mut Self> {
        validate_task(&task)?;
        if task.name != name {
            return Err(Error::new(format!(
                "replacement task name {:?} does not match {name:?}",
                task.name
            )));
        }
        if !self.tasks.contains_key(name) {
            return Err(Error::new(format!("cannot replace unknown task {name:?}")));
        }
        let task = if name == "output" {
            task.handles_output()
        } else {
            task
        };
        self.tasks.insert(name.to_owned(), task);
        Ok(self)
    }

    pub fn include(&mut self, namespace: &str, other: Registry) -> Result<&mut Self> {
        if !namespace.is_empty() && !valid_name(namespace) {
            return Err(Error::new("invalid task namespace"));
        }
        let tasks: Vec<_> = other
            .tasks
            .into_values()
            .filter(|task| !task.builtin)
            .map(|mut task| {
                if !namespace.is_empty() {
                    task.name = format!("{namespace}:{}", task.name);
                }
                task
            })
            .collect();
        for task in &tasks {
            if self.tasks.contains_key(&task.name) {
                return Err(Error::new(format!("duplicate task {:?}", task.name)));
            }
        }
        for task in tasks {
            self.tasks.insert(task.name.clone(), task);
        }
        Ok(self)
    }

    pub fn tasks(&self) -> impl Iterator<Item = &Task> {
        self.tasks.values()
    }

    /// Create an isolated execution context with this registry and a project root.
    pub fn context(self, root: impl Into<PathBuf>) -> Context {
        Context::new(root.into(), Arc::new(self))
    }

    pub(crate) fn plan(&self, tokens: &[String]) -> Result<Vec<Invocation>> {
        let mut invocations = Vec::new();
        let mut position = 0;
        while let Some(name) = tokens.get(position) {
            if name == "::" {
                return Err(Error::new("expected a task name, found a chain separator"));
            }
            let task = self.tasks.get(name).ok_or_else(|| {
                Error::new(format!(
                    "unknown task {name:?}; use --list to see available tasks"
                ))
            })?;
            let (arguments, consumed) =
                Arguments::extract(&task.parameters, &tokens[position + 1..])
                    .map_err(|error| Error::new(format!("{name}: {error}")))?;
            invocations.push(Invocation {
                name: name.clone(),
                arguments,
            });
            position += 1 + consumed;
            if tokens.get(position).is_some_and(|token| token == "::") {
                position += 1;
                if position == tokens.len() {
                    return Err(Error::new("expected a task after ::"));
                }
            }
        }
        Ok(invocations)
    }

    pub fn help(&self, name: Option<&str>) -> Result<String> {
        if let Some(name) = name {
            let task = self
                .tasks
                .get(name)
                .ok_or_else(|| Error::new(format!("unknown task {name:?}")))?;
            let mut text = format!("{}\n\n{}\n\nArguments:\n", task.name, task.description);
            for parameter in &task.parameters {
                let kind = if parameter.positional {
                    "positional"
                } else {
                    "named"
                };
                let display_name = if parameter.positional {
                    parameter.name.clone()
                } else {
                    format!("--{} value", parameter.name.replace('_', "-"))
                };
                let requirement = if parameter.required {
                    "required"
                } else {
                    "optional"
                };
                let default = parameter
                    .default
                    .as_ref()
                    .map(|value| format!(", default: {value}"))
                    .unwrap_or_default();
                let repeated = if parameter.repeated {
                    ", repeatable"
                } else {
                    ""
                };
                text.push_str(&format!(
                    "  {}: {} ({kind}, {requirement}{default}{repeated})\n",
                    display_name, parameter.type_name
                ));
                if !parameter.description.is_empty() {
                    text.push_str(&format!("    {}\n", parameter.description));
                }
            }
            Ok(text)
        } else {
            let mut text = String::from("Tasks:\n");
            for task in self.tasks.values() {
                text.push_str(&format!(
                    "  {:24} {}\n",
                    task.name,
                    task.description.lines().next().unwrap_or_default()
                ));
            }
            text.push_str("\nUse TASK --help for arguments. Chain tasks with ::.\n");
            Ok(text)
        }
    }

    /// Run the process command line and print its final result through `output`.
    pub fn run(self) -> Result<()> {
        let tokens: Vec<_> = std::env::args_os()
            .skip(1)
            .map(task_argument)
            .collect::<Result<_>>()?;
        let root =
            root_from_environment(std::env::var_os("BAKE_PROJECT_ROOT"), std::env::current_dir)?;
        self.run_with(root, &tokens, &mut io::stdout().lock())
    }

    fn run_with(self, root: PathBuf, tokens: &[String], writer: &mut dyn Write) -> Result<()> {
        let output = self.run_arguments(root, tokens)?;
        writer.write_all(output.as_bytes())?;
        Ok(())
    }

    /// Execute a command line without writing directly to stdout. Parsing and
    /// type validation finish before any task starts. Tasks may write an output
    /// file or return captured output.
    pub fn run_arguments(self, root: impl Into<PathBuf>, tokens: &[String]) -> Result<String> {
        self.run_arguments_from(root.into(), tokens)
    }

    fn run_arguments_from(self, root: PathBuf, tokens: &[String]) -> Result<String> {
        let (format, tokens) = if tokens.first().is_some_and(|token| token == "--json") {
            (Some(Format::Json), &tokens[1..])
        } else {
            (None, tokens)
        };
        if tokens.is_empty() || matches!(tokens, [flag] if flag == "--list" || flag == "--help") {
            return self.help(None);
        }
        if let [name, flag] = tokens
            && flag == "--help"
        {
            return self.help(Some(name));
        }
        let invocations = self.plan(tokens)?;
        let final_task_handles_output = invocations
            .last()
            .and_then(|invocation| self.tasks.get(&invocation.name))
            .is_some_and(Task::produces_output);
        let mut context = self.context(root);
        context.set_default_format(format);
        for invocation in invocations {
            context.invoke(invocation)?;
        }
        if !final_task_handles_output {
            context.invoke(Invocation {
                name: "output".into(),
                arguments: Arguments::default(),
            })?;
        }
        Ok(context.take_output())
    }
}

fn root_from_environment(
    configured_root: Option<std::ffi::OsString>,
    current_directory: fn() -> io::Result<PathBuf>,
) -> Result<PathBuf> {
    configured_root
        .map(PathBuf::from)
        .map_or_else(current_directory, Ok)
        .map_err(Into::into)
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

fn task_argument(token: OsString) -> Result<String> {
    token
        .into_string()
        .map_err(|_| Error::new("task arguments must be UTF-8"))
}

fn validate_task(task: &Task) -> Result<()> {
    if !valid_name(&task.name) {
        return Err(Error::new(format!("invalid task name {:?}", task.name)));
    }
    let mut parameters = BTreeSet::new();
    for parameter in &task.parameters {
        if parameter.name.is_empty()
            || !parameter
                .name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(Error::new(format!(
                "invalid parameter name {:?}",
                parameter.name
            )));
        }
        if !parameters.insert(&parameter.name) {
            return Err(Error::new(format!(
                "duplicate parameter {:?}",
                parameter.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handler(_: &mut Context, _: &Arguments) -> Result<crate::Value> {
        Ok(crate::Value::Null)
    }

    fn failing_handler(_: &mut Context, _: &Arguments) -> Result<crate::Value> {
        Err(Error::new("output failure"))
    }

    fn invalid_task() -> Task {
        task("invalid task name", vec![])
    }

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("writer failed"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn task(name: &str, parameters: Vec<crate::Parameter>) -> Task {
        Task::new(name, "A test task.", parameters, handler)
    }

    fn inspection_task() -> Task {
        task(
            "inspect",
            vec![
                crate::Parameter::new::<String>("name").help("A person's name."),
                crate::Parameter::new::<bool>("excited")
                    .named()
                    .default("false")
                    .help("Add emphasis."),
                crate::Parameter::new::<String>("label")
                    .repeated()
                    .help("Repeat this option."),
            ],
        )
    }

    #[test]
    fn replacement_checks_names_and_marks_output_handlers() {
        let mut registry = Registry::new();

        assert!(registry.replace("output", task("wrong", vec![])).is_err());
        assert!(registry.replace("invalid", invalid_task()).is_err());
        assert!(
            registry
                .replace("missing", task("missing", vec![]))
                .is_err()
        );

        registry.register(task("custom", vec![])).unwrap();
        registry.replace("custom", task("custom", vec![])).unwrap();
        registry.replace("output", task("output", vec![])).unwrap();

        assert!(registry.tasks["output"].produces_output());
        assert_eq!(
            registry
                .context(PathBuf::from("."))
                .call("custom", &[])
                .unwrap(),
            crate::Value::Null
        );
    }

    #[test]
    fn default_registry_includes_builtins_and_rejects_invalid_metadata() {
        let mut registry = Registry::default();
        assert!(registry.tasks.contains_key("output"));
        assert!(registry.tasks.contains_key("null"));

        assert!(
            registry
                .include("invalid namespace", Registry::new())
                .is_err()
        );
        let invalid_parameter = crate::Parameter::new::<String>("invalid-name");
        assert!(
            registry
                .register(task("valid", vec![invalid_parameter]))
                .is_err()
        );

        let duplicate_parameters = vec![
            crate::Parameter::new::<String>("name"),
            crate::Parameter::new::<String>("name"),
        ];
        assert!(
            registry
                .register(task("duplicate-parameters", duplicate_parameters))
                .is_err()
        );

        registry.register(task("duplicate", vec![])).unwrap();
        assert!(registry.register(task("duplicate", vec![])).is_err());
    }

    #[test]
    fn discovers_tasks_and_includes_namespaces() {
        let discovered = Registry::discover().unwrap();
        assert!(discovered.tasks.contains_key("output"));
        assert!(discovered.tasks.contains_key("null"));
        assert!(discovered.tasks().count() >= 2);

        let mut source = Registry::new();
        source.register(task("inspect", vec![])).unwrap();
        let mut registry = Registry::new();
        registry.include("project", source).unwrap();
        assert!(registry.tasks.contains_key("project:inspect"));

        let mut unnamespaced = Registry::new();
        unnamespaced.register(task("unqualified", vec![])).unwrap();
        registry.include("", unnamespaced).unwrap();
        assert!(registry.tasks.contains_key("unqualified"));

        let mut duplicate = Registry::new();
        duplicate.register(task("inspect", vec![])).unwrap();
        assert!(registry.include("project", duplicate).is_err());
    }

    #[test]
    fn run_prints_help_when_no_task_is_given() {
        assert!(Registry::new().run().is_ok());
    }

    #[test]
    fn runs_path_root_commands_and_reports_unknown_help_tasks() {
        let mut registry = Registry::new();
        registry.register(inspection_task()).unwrap();

        let help = registry.help(Some("inspect")).unwrap();
        assert!(help.contains("name: alloc::string::String (positional, required)"));
        assert!(help.contains("--excited value: bool (named, optional, default: false)"));
        assert!(
            help.contains("--label value: alloc::string::String (named, optional, repeatable)")
        );
        assert!(help.contains("Repeat this option."));

        assert_eq!(
            registry
                .run_arguments(
                    PathBuf::from("."),
                    &["--json".into(), "inspect".into(), "Sam".into()],
                )
                .unwrap(),
            "null\n"
        );

        let mut invalid_arguments_registry = Registry::new();
        invalid_arguments_registry
            .register(inspection_task())
            .unwrap();
        assert!(
            invalid_arguments_registry
                .run_arguments(PathBuf::from("."), &["inspect".into()])
                .unwrap_err()
                .to_string()
                .contains("inspect: missing argument \"name\"")
        );

        let mut help_registry = Registry::new();
        help_registry.register(inspection_task()).unwrap();
        assert_eq!(
            help_registry
                .run_arguments(PathBuf::from("."), &["inspect".into(), "--help".into()])
                .unwrap(),
            help
        );
        assert!(Registry::new().help(Some("missing")).is_err());
    }

    #[test]
    fn converts_process_arguments_and_rejects_non_utf8_values() {
        assert_eq!(task_argument("inspect".into()).unwrap(), "inspect");

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;

            assert!(task_argument(OsString::from_vec(vec![0xff])).is_err());
        }
    }

    #[test]
    fn propagates_discovery_output_and_writer_errors() {
        static INVALID_REGISTRATIONS: [TaskRegistration; 1] = [TaskRegistration {
            factory: invalid_task,
            builtin: false,
        }];
        assert!(Registry::discover_from(&INVALID_REGISTRATIONS).is_err());
        let duplicate_registrations = [
            TaskRegistration {
                factory: inspection_task,
                builtin: false,
            },
            TaskRegistration {
                factory: inspection_task,
                builtin: false,
            },
        ];
        assert!(
            Registry::discover_from(&duplicate_registrations)
                .err()
                .unwrap()
                .to_string()
                .contains("duplicate task")
        );

        let mut registry = Registry::new();
        registry.register(task("inspect", vec![])).unwrap();
        registry
            .replace(
                "output",
                Task::new("output", "Fail.", vec![], failing_handler),
            )
            .unwrap();
        assert!(registry.run_arguments(".", &["inspect".into()]).is_err());

        let mut failing_writer = FailingWriter;
        assert!(
            Registry::new()
                .run_with(PathBuf::from("."), &[], &mut failing_writer)
                .is_err()
        );
        assert!(failing_writer.flush().is_ok());
        assert!(
            root_from_environment(None, || Err(io::Error::other("no current directory"))).is_err()
        );
        assert_eq!(
            root_from_environment(Some("project".into()), std::env::current_dir).unwrap(),
            PathBuf::from("project")
        );
    }
}
