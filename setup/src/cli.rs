//! The command line of contract 12.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::commands::remove::UserData;

pub const USAGE: &str = "usage: crownos-android-setup run
       crownos-android-setup prepare-container
       crownos-android-setup remove [--wipe-data]
       crownos-android-setup enable
       crownos-android-setup make-manifest --root <dir> --version <v> --pins <json>";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Run,
    PrepareContainer,
    Remove(UserData),
    Enable,
    MakeManifest {
        root: PathBuf,
        version: String,
        pins: PathBuf,
    },
}

#[derive(Debug, thiserror::Error)]
#[error("{0}\n{USAGE}")]
pub struct UsageError(String);

/// Parses the arguments after the program name.
pub fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, UsageError> {
    let mut args = args.into_iter();
    let command = args
        .next()
        .ok_or_else(|| UsageError("no command given".into()))?;
    let rest: Vec<OsString> = args.collect();
    match (command.to_str(), rest.as_slice()) {
        (Some("run"), []) => Ok(Command::Run),
        (Some("prepare-container"), []) => Ok(Command::PrepareContainer),
        (Some("remove"), []) => Ok(Command::Remove(UserData::Keep)),
        (Some("remove"), [flag]) if flag == "--wipe-data" => Ok(Command::Remove(UserData::Wipe)),
        (Some("enable"), []) => Ok(Command::Enable),
        (Some("make-manifest"), _) => make_manifest(rest),
        (Some("run" | "prepare-container" | "remove" | "enable"), _) => {
            Err(UsageError(format!("unexpected arguments {rest:?}")))
        }
        _ => Err(UsageError(format!("unknown command {command:?}"))),
    }
}

fn make_manifest(rest: Vec<OsString>) -> Result<Command, UsageError> {
    let (mut root, mut version, mut pins) = (None, None, None);
    let mut args = rest.into_iter();
    while let Some(option) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| UsageError(format!("{option:?} needs a value")))?;
        match option.to_str() {
            Some("--root") => root = Some(PathBuf::from(value)),
            Some("--pins") => pins = Some(PathBuf::from(value)),
            Some("--version") => {
                version = Some(
                    value
                        .into_string()
                        .map_err(|_| UsageError("the version is not UTF-8".into()))?,
                );
            }
            _ => return Err(UsageError(format!("unknown option {option:?}"))),
        }
    }
    match (root, version, pins) {
        (Some(root), Some(version), Some(pins)) => Ok(Command::MakeManifest {
            root,
            version,
            pins,
        }),
        _ => Err(UsageError(
            "make-manifest needs --root, --version and --pins".into(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_words(words: &str) -> Result<Command, UsageError> {
        parse(words.split_whitespace().map(OsString::from))
    }

    #[test]
    fn every_contract_command_parses() -> Result<(), UsageError> {
        assert_eq!(parse_words("run")?, Command::Run);
        assert_eq!(parse_words("prepare-container")?, Command::PrepareContainer);
        assert_eq!(parse_words("remove")?, Command::Remove(UserData::Keep));
        assert_eq!(
            parse_words("remove --wipe-data")?,
            Command::Remove(UserData::Wipe)
        );
        assert_eq!(parse_words("enable")?, Command::Enable);
        assert_eq!(
            parse_words("make-manifest --version 2026.11.0 --root /b --pins pins.json")?,
            Command::MakeManifest {
                root: "/b".into(),
                version: "2026.11.0".into(),
                pins: "pins.json".into(),
            }
        );
        Ok(())
    }

    #[test]
    fn anything_else_is_a_usage_error() {
        for words in [
            "",
            "install",
            "run now",
            "remove --all",
            "enable now",
            "make-manifest --root /b",
            "make-manifest --root",
        ] {
            assert!(parse_words(words).is_err(), "{words:?}");
        }
    }
}
