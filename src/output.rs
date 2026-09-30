// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{Context, Error, Result, Value};
use std::fmt;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;

/// Built-in result encodings accepted by the default `output` task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    /// Plain text for strings and pretty JSON for structured values.
    Raw,
    /// Indented JSON.
    Json,
    /// One compact JSON value per line; the input must be an array.
    Ndjson,
}

impl FromStr for Format {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "raw" => Ok(Self::Raw),
            "json" => Ok(Self::Json),
            "ndjson" => Ok(Self::Ndjson),
            _ => Err(Error::new(format!(
                "unknown output format {value:?}; use raw, json, or ndjson"
            ))),
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Raw => "raw",
            Self::Json => "json",
            Self::Ndjson => "ndjson",
        })
    }
}

fn format_value(value: &Value, format: Format) -> Result<String> {
    let mut output = match format {
        Format::Raw => match value {
            Value::String(text) => text.clone(),
            Value::Null => String::new(),
            value => serde_json::to_string_pretty(value)?,
        },
        Format::Json => serde_json::to_string_pretty(value)?,
        Format::Ndjson => {
            let Value::Array(values) = value else {
                return Err(Error::new("ndjson output requires an array result"));
            };
            let mut output = String::new();
            for value in values {
                output.push_str(&serde_json::to_string(value)?);
                output.push('\n');
            }
            return Ok(output);
        }
    };

    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    Ok(output)
}

fn inferred_format(file: &Path) -> Option<Format> {
    match file.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "json" => Some(Format::Json),
        "ndjson" => Some(Format::Ndjson),
        "txt" | "text" => Some(Format::Raw),
        _ => None,
    }
}

/// Format or write the value returned by the previous task.
///
/// With no explicit `format`, JSON and NDJSON filename extensions select those
/// formats. Other destinations use raw output. The input value is returned so
/// further chained tasks can inspect it.
#[crate::task(output, builtin)]
pub fn output(
    context: &mut Context,
    #[bake(input)] input: Value,
    file: Option<PathBuf>,
    format: Option<Format>,
) -> Result<Value> {
    let format = format
        .or_else(|| context.default_format())
        .or_else(|| file.as_deref().and_then(inferred_format))
        .unwrap_or(Format::Raw);
    let contents = format_value(&input, format)?;

    if let Some(file) = file {
        let path = context.root().join(file);
        fs::write(&path, contents.as_bytes())
            .map_err(|error| Error::new(format!("{}: {error}", path.display())))?;
    } else {
        context.write_output(&contents);
    }

    Ok(input)
}

/// Consume a previous result without writing it.
#[crate::task(output, builtin)]
pub fn null(#[bake(input)] input: Value) -> Result<Value> {
    Ok(input)
}

pub(crate) fn builtins() -> Vec<crate::Task> {
    vec![output_task().builtin(), null_task().builtin()]
}
