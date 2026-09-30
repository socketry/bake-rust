use crate::{Arguments, Context, Parameter, Result, Value};

/// A function's command metadata and generated invocation adapter.
pub struct Task {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) invoke: fn(&mut Context, &Arguments) -> Result<Value>,
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
        }
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
}
