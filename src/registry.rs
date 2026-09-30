use crate::{Arguments, Context, Error, Format, Result, Task, output};
use std::collections::{BTreeMap, BTreeSet};
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
    pub module_path: &'static str,
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
    ///
    /// A dependency that contributes tasks must be referenced by the executable
    /// (for example, `use bake_releases as _;`) so the linker includes it.
    pub fn discover() -> Result<Self> {
        let mut registry = Self::new();
        for registration in TASK_REGISTRATIONS {
            if registration.builtin {
                continue;
            }
            let mut task = (registration.factory)();
            namespace_from_module(&mut task, registration.module_path);
            registry.register(task)?;
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
            .map(|token| {
                token
                    .into_string()
                    .map_err(|_| Error::new("task arguments must be UTF-8"))
            })
            .collect::<Result<_>>()?;
        let root = std::env::var_os("BAKE_PROJECT_ROOT")
            .map(PathBuf::from)
            .map_or_else(std::env::current_dir, Ok)?;
        let output = self.run_arguments(root, &tokens)?;
        io::stdout().lock().write_all(output.as_bytes())?;
        Ok(())
    }

    /// Execute a command line without writing directly to stdout. Parsing and
    /// type validation finish before any task starts. Tasks may write an output
    /// file or return captured output.
    pub fn run_arguments(self, root: impl Into<PathBuf>, tokens: &[String]) -> Result<String> {
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

fn namespace_from_module(task: &mut Task, module_path: &str) {
    if task.name.contains(':') {
        return;
    }

    let namespace = module_path
        .split("::")
        .skip(1)
        .map(|component| component.replace('_', "-"))
        .collect::<Vec<_>>()
        .join(":");

    if !namespace.is_empty() {
        task.name = format!("{namespace}:{}", task.name);
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
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
