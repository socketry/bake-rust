// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::Result;
use crate::arguments::parse;
use std::fmt::Display;
use std::str::FromStr;

/// A task parameter. Names use underscores; command-line flags also accept hyphens.
pub struct Parameter {
    pub(crate) name: String,
    pub(crate) type_name: &'static str,
    pub(crate) positional: bool,
    pub(crate) required: bool,
    pub(crate) repeated: bool,
    pub(crate) default: Option<String>,
    pub(crate) description: String,
    pub(crate) validate: fn(&str) -> Result<()>,
}

impl Parameter {
    /// Values use `FromStr`; custom types can implement it too.
    pub fn new<Value: FromStr>(name: &str) -> Self
    where
        Value::Err: Display,
    {
        Self {
            name: name.into(),
            type_name: std::any::type_name::<Value>(),
            positional: true,
            required: true,
            repeated: false,
            default: None,
            description: String::new(),
            validate: |value| parse::<Value>(value).map(|_| ()),
        }
    }
    pub fn named(mut self) -> Self {
        self.positional = false;
        self
    }
    pub fn optional(mut self) -> Self {
        self.required = false;
        self
    }
    pub fn repeated(mut self) -> Self {
        self.repeated = true;
        self.positional = false;
        self.required = false;
        self
    }
    /// Consume repeated bare values until the task chain separator.
    pub fn variadic(mut self) -> Self {
        self.repeated = true;
        self.positional = true;
        self.required = false;
        self
    }
    pub fn default(mut self, value: impl Into<String>) -> Self {
        self.default = Some(value.into());
        self.required = false;
        self.positional = false;
        self
    }
    pub fn help(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}
