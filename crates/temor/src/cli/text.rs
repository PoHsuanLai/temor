//! The words the subcommands print. No jargon: a person reads these.

use std::io::ErrorKind;

use temor_transfer::TransferError;
use temor_transfer::kitty::REFUSED_BY_PERSON;

pub const USAGE: &str = "\
temor save <file>...   Send files to your Downloads shelf
temor get [name]       Bring a file from your shelf into this folder
temor view <file>      Open a file with its viewer
temor -e <program>...  Run a program in a new tab
temor --version        Show which temor this is";

/// A transfer error in plain language.
pub fn transfer_words(err: &TransferError) -> String {
    match err {
        TransferError::NoAnswer => "Nothing answered. This works only inside a temor window, and \
                                    that window has to be the one you are typing in"
            .to_owned(),
        TransferError::Declined { message } if message == REFUSED_BY_PERSON => {
            "You said no in temor, so nothing was sent".to_owned()
        }
        TransferError::Declined { message } => format!("temor couldn't finish: {message}"),
        TransferError::Link(_) => "The connection to the terminal broke".to_owned(),
        TransferError::NotAFile { path } => format!(
            "{} is not an ordinary file. Pick a file, not a folder",
            path.display()
        ),
        TransferError::Io { path, source } => {
            let why = match source.kind() {
                ErrorKind::NotFound => "there is no such file".to_owned(),
                ErrorKind::PermissionDenied => "you don't have permission".to_owned(),
                ErrorKind::AlreadyExists => "a file with that name is already there".to_owned(),
                _ => source.to_string(),
            };
            format!("Couldn't use {}: {why}", path.display())
        }
        TransferError::TooLarge { .. } | TransferError::TooManyEntries { .. } => {
            "That is more than temor takes in one go".to_owned()
        }
        TransferError::UnsafePath { path } => format!("The name {path} isn't allowed"),
        TransferError::Unsupported { .. } => "This isn't supported on this computer yet".to_owned(),
        _ => "The other side sent something temor didn't understand".to_owned(),
    }
}

/// What `temor save` says when it is done.
pub fn saved_words(count: usize) -> String {
    match count {
        1 => "Sent 1 file to your shelf".to_owned(),
        n => format!("Sent {n} files to your shelf"),
    }
}

/// What `temor get` says when it is done.
pub fn got_words(names: &[String]) -> String {
    match names {
        [one] => format!("Saved {one} in this folder"),
        many => format!("Saved {} files in this folder", many.len()),
    }
}

/// What `temor view` says over SSH, where the viewer is on the person's own computer.
pub const VIEW_SENT: &str = "Sent to your shelf. Open it from there, on your own computer";
