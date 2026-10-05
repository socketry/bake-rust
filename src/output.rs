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
            value => pretty_json(value),
        },
        Format::Json => pretty_json(value),
        Format::Ndjson => {
            let Value::Array(values) = value else {
                return Err(Error::new("ndjson output requires an array result"));
            };
            let mut output = String::new();
            for value in values {
                output.push_str(&compact_json(value));
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

fn pretty_json(value: &Value) -> String {
    serde_json::to_string_pretty(value)
        .unwrap_or_else(|error| unreachable!("JSON values always serialize: {error}"))
}

fn compact_json(value: &Value) -> String {
    serde_json::to_string(value)
        .unwrap_or_else(|error| unreachable!("JSON values always serialize: {error}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_have_stable_names() {
        assert_eq!(Format::Raw.to_string(), "raw");
        assert_eq!(Format::Json.to_string(), "json");
        assert_eq!(Format::Ndjson.to_string(), "ndjson");
        assert_eq!("raw".parse::<Format>().unwrap(), Format::Raw);
        assert_eq!("json".parse::<Format>().unwrap(), Format::Json);
        assert_eq!("ndjson".parse::<Format>().unwrap(), Format::Ndjson);
        assert!("yaml".parse::<Format>().is_err());
    }

    #[test]
    fn unknown_file_extensions_do_not_select_a_format() {
        assert_eq!(inferred_format(Path::new("releases.md")), None);
        assert_eq!(inferred_format(Path::new("notes.csv")), None);
        assert_eq!(inferred_format(Path::new("notes.txt")), Some(Format::Raw));
        assert_eq!(inferred_format(Path::new("notes.text")), Some(Format::Raw));
        assert_eq!(
            inferred_format(Path::new("notes.ndjson")),
            Some(Format::Ndjson)
        );
    }

    #[test]
    fn non_utf8_file_extensions_do_not_select_a_format() {
        #[cfg(unix)]
        let path = {
            use std::os::unix::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_vec(vec![
                b'n', b'o', b't', b'e', b'.', 0xff,
            ]))
        };
        #[cfg(windows)]
        let path = {
            use std::os::windows::ffi::OsStringExt;
            PathBuf::from(std::ffi::OsString::from_wide(&[
                b'n' as u16,
                b'o' as u16,
                b't' as u16,
                b'e' as u16,
                b'.' as u16,
                0xd800,
            ]))
        };
        assert_eq!(inferred_format(&path), None);
    }

    #[test]
    fn reports_output_file_write_errors() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("directory");
        fs::create_dir(&destination).unwrap();
        let mut context = crate::Registry::new().context(directory.path());

        assert!(output(&mut context, Value::Null, Some(destination), None).is_err());
    }
}
