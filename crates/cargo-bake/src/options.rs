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
    pub(crate) help: bool,
    pub(crate) version: bool,
    pub(crate) arguments: Vec<OsString>,
}

impl Options {
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self> {
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
                Some("--help" | "-h") => options.help = true,
                Some("--version" | "-V") => options.version = true,
                Some("--") => {
                    options.arguments.extend(arguments);
                    break;
                }
                Some(value) if value.starts_with("--manifest-path=") => {
                    options.manifest = Some(value["--manifest-path=".len()..].into());
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
