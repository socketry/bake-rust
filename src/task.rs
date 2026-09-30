use crate::{Arguments, Context, Parameter, Result, Value};

/// A function's command metadata and generated invocation adapter.
pub struct Task {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) invoke: fn(&mut Context, &Arguments) -> Result<Value>,
    pub(crate) handles_output: bool,
    pub(crate) builtin: bool,
}

impl Task {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Vec<Parameter>,
        invoke: fn(&mut Context, &Arguments) -> Result<Value>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
            invoke,
            handles_output: false,
            builtin: false,
        }
    }

    /// Mark a task as responsible for its own user-facing output.
    pub fn handles_output(mut self) -> Self {
        self.handles_output = true;
        self
    }

    pub(crate) fn builtin(mut self) -> Self {
        self.builtin = true;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn parameters(&self) -> &[Parameter] {
        &self.parameters
    }
    pub fn produces_output(&self) -> bool {
        self.handles_output
    }
}
