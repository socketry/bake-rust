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
}

impl Context {
    pub(crate) fn new(root: PathBuf, registry: Arc<Registry>) -> Self {
        Self {
            root,
            registry,
            previous: Value::Null,
            state: HashMap::new(),
            depth: 0,
        }
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn previous(&self) -> &Value {
        &self.previous
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
