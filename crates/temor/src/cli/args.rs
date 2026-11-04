//! Reading the command line. Pure.

use std::ffi::OsString;
use std::path::PathBuf;

/// What the person asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Send files to the shelf.
    Save { files: Vec<PathBuf> },
    /// Pull a file from the shelf into the current folder; the newest when no name is given.
    Get { name: Option<String> },
    /// Show a file with its viewer.
    View { file: PathBuf },
    /// Print how to use it.
    Help,
    /// Print which temor this is.
    Version,
}

/// Why the command line did not make sense.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UsageError {
    #[error("Tell me which file to send, like: temor save report.pdf")]
    NothingToSave,
    #[error("Tell me which file to open, like: temor view report.pdf")]
    NothingToView,
    #[error("I can open one file at a time")]
    OneFileToView,
    #[error("I can fetch one file at a time")]
    OneFileToGet,
    #[error("I don't know the option {0}")]
    UnknownOption(String),
    #[error("The file name has to be plain text")]
    NameNotText,
}

/// Reads `args` (the program name first). `None` when the first word is not one of ours, so the
/// window starts as usual.
pub fn parse(args: &[OsString]) -> Option<Result<Command, UsageError>> {
    let word = args.get(1)?.to_str()?;
    let rest = &args[2..];
    match word {
        "save" => Some(save(rest)),
        "get" => Some(get(rest)),
        "view" => Some(view(rest)),
        "-h" | "--help" => Some(Ok(Command::Help)),
        "-V" | "--version" => Some(Ok(Command::Version)),
        _ => None,
    }
}

/// The words after the subcommand with options taken out (`--` ends options), and whether help
/// was asked for.
fn operands(rest: &[OsString]) -> Result<(Vec<&OsString>, bool), UsageError> {
    let mut out = Vec::new();
    let mut help = false;
    let mut options_over = false;
    for arg in rest {
        let text = arg.to_str();
        match (options_over, text) {
            (false, Some("--")) => options_over = true,
            (false, Some("-h" | "--help")) => help = true,
            (false, Some(option)) if option.starts_with('-') && option != "-" => {
                return Err(UsageError::UnknownOption(option.to_owned()));
            }
            _ => out.push(arg),
        }
    }
    Ok((out, help))
}

fn save(rest: &[OsString]) -> Result<Command, UsageError> {
    let (files, help) = operands(rest)?;
    if help {
        return Ok(Command::Help);
    }
    if files.is_empty() {
        return Err(UsageError::NothingToSave);
    }
    Ok(Command::Save {
        files: files.into_iter().map(PathBuf::from).collect(),
    })
}

fn get(rest: &[OsString]) -> Result<Command, UsageError> {
    let (names, help) = operands(rest)?;
    if help {
        return Ok(Command::Help);
    }
    match names.as_slice() {
        [] => Ok(Command::Get { name: None }),
        [one] => one
            .to_str()
            .map(|name| Command::Get {
                name: Some(name.to_owned()),
            })
            .ok_or(UsageError::NameNotText),
        _ => Err(UsageError::OneFileToGet),
    }
}

fn view(rest: &[OsString]) -> Result<Command, UsageError> {
    let (files, help) = operands(rest)?;
    if help {
        return Ok(Command::Help);
    }
    match files.as_slice() {
        [] => Err(UsageError::NothingToView),
        [one] => Ok(Command::View {
            file: PathBuf::from(one),
        }),
        _ => Err(UsageError::OneFileToView),
    }
}
