// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::{Error, Result};
use std::fmt;
use std::str::FromStr;

/// Built-in result encodings accepted by the default `output` task.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputFormat {
    /// Plain text for strings and pretty JSON for structured values.
    Raw,
    /// Indented JSON.
    Json,
    /// One compact JSON value per line; the input must be an array.
    Ndjson,
}

impl FromStr for OutputFormat {
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

impl fmt::Display for OutputFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Raw => "raw",
            Self::Json => "json",
            Self::Ndjson => "ndjson",
        })
    }
}
