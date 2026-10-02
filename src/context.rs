// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::output::Format;
use crate::registry::Invocation;
use crate::{Error, Registry, Result, Value};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

/// Execution state shared by a chain of tasks and their nested calls.
///
/// A context is synchronous and confined to its caller. No global current
/// directory or environment changes are needed for task execution.
pub struct Context {
    root: PathBuf,
    registry: Arc<Registry>,
    previous: Value,
    state: HashMap<TypeId, Box<dyn Any>>,
    depth: usize,
    terminal_output: String,
    default_format: Option<Format>,
}

impl Context {
    pub(crate) fn new(root: PathBuf, registry: Arc<Registry>) -> Self {
        Self {
            root,
            registry,
            previous: Value::Null,
            state: HashMap::new(),
            depth: 0,
            terminal_output: String::new(),
            default_format: None,
        }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn previous(&self) -> &Value {
        &self.previous
    }
    /// Add user-facing output to the command result. Task diagnostics should use stderr.
    pub fn write_output(&mut self, output: &str) {
        self.terminal_output.push_str(output);
    }
    pub(crate) fn take_output(&mut self) -> String {
        std::mem::take(&mut self.terminal_output)
    }
    pub(crate) fn set_default_format(&mut self, format: Option<Format>) {
        self.default_format = format;
    }
    pub(crate) fn default_format(&self) -> Option<Format> {
        self.default_format
    }
    pub fn insert<State: Any>(&mut self, state: State) {
        self.state.insert(TypeId::of::<State>(), Box::new(state));
    }
    pub fn get<State: Any>(&self) -> Option<&State> {
        self.state.get(&TypeId::of::<State>())?.downcast_ref()
    }
    pub fn get_mut<State: Any>(&mut self) -> Option<&mut State> {
        self.state.get_mut(&TypeId::of::<State>())?.downcast_mut()
    }

    /// Start a process in the project root. Arguments go directly to the process;
    /// shell parsing is only used if a task explicitly invokes a shell.
    pub fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let mut command = Command::new(program);
        command.current_dir(&self.root);
        command
    }

    /// Invoke one registered task and store its successful output as `previous`.
    /// Explicit calls support composition and project-defined hooks.
    pub fn call(&mut self, name: &str, arguments: &[&str]) -> Result<Value> {
        let tokens: Vec<_> = std::iter::once(name)
            .chain(arguments.iter().copied())
            .map(str::to_owned)
            .collect();
        let mut invocations = self.registry.plan(&tokens)?;
        if invocations.len() != 1 {
            return Err(Error::new("Context::call accepts exactly one task"));
        }
        self.invoke(invocations.remove(0))
    }

    /// Invoke a registered task if it exists, returning `None` when it is absent.
    /// Errors from a registered task are returned unchanged.
    pub fn call_if_registered(&mut self, name: &str, arguments: &[&str]) -> Result<Option<Value>> {
        if !self.registry.tasks.contains_key(name) {
            return Ok(None);
        }

        self.call(name, arguments).map(Some)
    }

    pub(crate) fn invoke(&mut self, invocation: Invocation) -> Result<Value> {
        if self.depth >= 64 {
            return Err(Error::new(
                "task call depth exceeded 64; check for recursive hooks",
            ));
        }
        let handler = self
            .registry
            .tasks
            .get(&invocation.name)
            .ok_or_else(|| Error::new(format!("unknown task {:?}", invocation.name)))?
            .invoke;
        self.depth += 1;
        let result = handler(self, &invocation.arguments);
        self.depth -= 1;
        let result = result.map_err(|error| Error::new(format!("{}: {error}", invocation.name)))?;
        self.previous = result.clone();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Invocation;
    use crate::{Arguments, Task};

    fn value_task(_: &mut Context, _: &Arguments) -> Result<Value> {
        Ok(Value::Null)
    }

    #[test]
    fn exposes_root_previous_output_and_typed_state() {
        let mut context = Registry::new().context("project");
        assert_eq!(context.root(), Path::new("project"));
        assert_eq!(context.previous(), &Value::Null);
        context.set_default_format(Some(Format::Json));
        assert_eq!(context.default_format(), Some(Format::Json));
        assert!(context.get::<usize>().is_none());
        assert!(context.get_mut::<usize>().is_none());

        context.insert(40usize);
        assert_eq!(context.get::<usize>(), Some(&40));
        *context.get_mut::<usize>().unwrap() += 2;
        assert_eq!(context.get::<usize>(), Some(&42));
        assert!(context.get::<String>().is_none());

        context.write_output("hello");
        assert_eq!(context.take_output(), "hello");
        assert_eq!(context.take_output(), "");
    }

    #[test]
    fn reports_invalid_and_unknown_task_calls() {
        let mut registry = Registry::new();
        registry
            .register(Task::new("value", "Return null.", Vec::new(), value_task))
            .unwrap();
        let mut context = registry.context(".");

        assert_eq!(context.call("value", &[]).unwrap(), Value::Null);
        assert!(context.call("missing", &[]).is_err());
        assert!(context.call("value", &["::", "value"]).is_err());
        assert!(
            context
                .call_if_registered("missing", &[])
                .unwrap()
                .is_none()
        );
        assert!(
            context
                .invoke(Invocation {
                    name: "missing".to_owned(),
                    arguments: Arguments::default(),
                })
                .is_err()
        );
    }
}
