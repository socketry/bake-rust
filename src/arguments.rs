use crate::{Error, Result};
use std::collections::BTreeMap;
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
    validate: fn(&str) -> Result<()>,
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

fn parse<Value: FromStr>(value: &str) -> Result<Value>
where
    Value::Err: Display,
{
    value.parse().map_err(|error| {
        Error::new(format!(
            "invalid {} value {value:?}: {error}",
            std::any::type_name::<Value>()
        ))
    })
}

/// Validated command-line values, converted to Rust types by the task adapter.
#[derive(Default, Debug)]
pub struct Arguments(BTreeMap<String, Vec<String>>);

impl Arguments {
    pub fn required<Value: FromStr>(&self, name: &str) -> Result<Value>
    where
        Value::Err: Display,
    {
        self.optional(name)?
            .ok_or_else(|| Error::new(format!("missing argument {name:?}")))
    }
    pub fn optional<Value: FromStr>(&self, name: &str) -> Result<Option<Value>>
    where
        Value::Err: Display,
    {
        self.0
            .get(name)
            .and_then(|values| values.first())
            .map(|value| parse(value))
            .transpose()
    }
    pub fn repeated<Value: FromStr>(&self, name: &str) -> Result<Vec<Value>>
    where
        Value::Err: Display,
    {
        self.0
            .get(name)
            .into_iter()
            .flatten()
            .map(|value| parse(value))
            .collect()
    }

    fn insert(&mut self, parameter: &Parameter, value: &str) -> Result<()> {
        if !parameter.repeated && self.0.contains_key(&parameter.name) {
            return Err(Error::new(format!(
                "argument {:?} was supplied more than once",
                parameter.name
            )));
        }
        (parameter.validate)(value)
            .map_err(|error| Error::new(format!("{}: {error}", parameter.name)))?;
        self.0
            .entry(parameter.name.clone())
            .or_default()
            .push(value.into());
        Ok(())
    }

    pub(crate) fn extract(parameters: &[Parameter], tokens: &[String]) -> Result<(Self, usize)> {
        let mut arguments = Self::default();
        let mut consumed = 0;
        let mut options = true;
        while let Some(token) = tokens.get(consumed) {
            if token == "::" {
                break;
            }
            if options && token == "--" {
                options = false;
                consumed += 1;
                continue;
            }
            let named = if options {
                token.strip_prefix("--")
            } else {
                None
            };
            if let Some(name) = named {
                if name.contains('=') {
                    return Err(Error::new(
                        "named arguments use separate `--name value` tokens",
                    ));
                }
                let name = name.replace('-', "_");
                let parameter = parameters
                    .iter()
                    .find(|parameter| parameter.name == name)
                    .ok_or_else(|| Error::new(format!("unknown argument {name:?}")))?;
                consumed += 1;
                let value = tokens
                    .get(consumed)
                    .filter(|value| value.as_str() != "::" && value.as_str() != "--")
                    .ok_or_else(|| Error::new(format!("argument {name:?} requires a value")))?;
                consumed += 1;
                arguments.insert(parameter, value)?;
            } else if let Some(parameter) = parameters.iter().find(|parameter| {
                parameter.positional && !arguments.0.contains_key(&parameter.name)
            }) {
                arguments.insert(parameter, token)?;
                consumed += 1;
            } else {
                break;
            }
        }
        for parameter in parameters {
            if parameter.required && !arguments.0.contains_key(&parameter.name) {
                return Err(Error::new(format!("missing argument {:?}", parameter.name)));
            }
        }
        Ok((arguments, consumed))
    }
}
