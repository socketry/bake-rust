// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

use crate::Result;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

#[derive(Default)]
pub(crate) struct Options {
    pub(crate) manifest: Option<PathBuf>,
    pub(crate) offline: bool,
    pub(crate) locked: bool,
    pub(crate) release: bool,
    pub(crate) regenerate: bool,
    pub(crate) help: bool,
    pub(crate) version: bool,
    pub(crate) arguments: Vec<OsString>,
}

impl Options {
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
        Self::parse_arguments(arguments.into_iter().collect())
    }

    fn parse_arguments(arguments: Vec<OsString>) -> Result<Self> {
        let mut arguments = arguments.into_iter().peekable();
        // Cargo external subcommands receive their command name as argument one.
        if arguments.peek().is_some_and(|argument| argument == "bake") {
            arguments.next();
        }
        let mut options = Self::default();
        while let Some(argument) = arguments.next() {
            match argument.to_str() {
                Some("--manifest-path") => {
                    options.manifest = Some(
                        arguments
                            .next()
                            .ok_or("--manifest-path requires a path")?
                            .into(),
                    );
                }
                Some("--offline") => options.offline = true,
                Some("--locked") => options.locked = true,
                Some("--release") => options.release = true,
                Some("--regenerate") => options.regenerate = true,
                Some("--help" | "-h") => options.help = true,
                Some("--version" | "-V") => options.version = true,
                Some("--") => {
                    options.arguments.extend(arguments);
                    break;
                }
                _ => {
                    options.arguments.push(argument);
                    options.arguments.extend(arguments);
                    break;
                }
            }
        }
        Ok(options)
    }

    pub(crate) fn configure(&self, command: &mut Command) {
        if self.offline {
            command.arg("--offline");
        }
        if self.locked {
            command.arg("--locked");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_tokens_after_a_separator_unchanged() {
        let options = Options::parse(
            ["bake", "--locked", "--", "--not-a-launcher-option", "value"].map(OsString::from),
        )
        .unwrap();

        assert!(options.locked);
        assert_eq!(options.arguments, ["--not-a-launcher-option", "value"]);
    }

    #[test]
    fn parses_launcher_options_and_rejects_missing_values() {
        let options = Options::parse(
            [
                "--manifest-path",
                "project/Cargo.toml",
                "--offline",
                "--locked",
                "--release",
                "--regenerate",
                "--version",
            ]
            .map(OsString::from),
        )
        .unwrap();

        assert_eq!(options.manifest, Some(PathBuf::from("project/Cargo.toml")));
        assert!(options.offline);
        assert!(options.locked);
        assert!(options.release);
        assert!(options.regenerate);
        assert!(options.version);
        assert!(Options::parse([OsString::from("--manifest-path")]).is_err());

        assert!(Options::parse([OsString::from("-h")]).unwrap().help);
        assert!(Options::parse([OsString::from("-V")]).unwrap().version);
    }

    #[cfg(unix)]
    #[test]
    fn forwards_non_utf8_task_arguments() {
        use std::os::unix::ffi::OsStringExt;

        let argument = OsString::from_vec(vec![0xff]);
        let options = Options::parse([argument.clone()]).unwrap();

        assert_eq!(options.arguments, [argument]);
    }

    #[test]
    fn forwards_offline_and_locked_options_to_cargo() {
        let options = Options {
            offline: true,
            locked: true,
            ..Options::default()
        };
        let mut command = Command::new("cargo");

        options.configure(&mut command);

        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["--offline", "--locked"]
        );
    }
}
