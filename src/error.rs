// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use std::fmt;

/// An actionable task, argument, registration, or process error.
#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::new(error.to_string())
    }
}

pub type Result<Output = ()> = std::result::Result<Output, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_io_and_json_errors() {
        let io_error = Error::from(std::io::Error::other("file unavailable"));
        assert_eq!(io_error.to_string(), "file unavailable");

        let json_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let error = Error::from(json_error);
        assert!(error.to_string().contains("EOF while parsing"));
    }
}
