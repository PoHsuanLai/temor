//! Carrying the commands out: the terminal line, the transfer drivers and the opener.

use std::path::PathBuf;
use std::time::Duration;

use temor_transfer::kitty::{Action, KittyId, Message, Waits, drive_receive, drive_send};
use temor_transfer::{Bytes, Count, Limits, Root, TransferError};

use super::args::Command;
use super::error::CliError;
use super::opener::Opener;
use super::platform::open_tty;
use super::text::{VIEW_SENT, got_words, saved_words};
use super::view::{ViewPlan, Where, plan};

/// What the command line knows about where it runs.
#[derive(Debug, Clone, Copy)]
pub struct Context {
    pub at: Where,
    pub pid: u32,
}

/// The longest status or frame read back: replies are short, and files are checked by temor.
fn limits() -> Limits {
    Limits::new(Bytes::new(1 << 30), Count::new(10_000), Count::new(64))
}

/// The terminal answers `STARTED` as soon as it has heard an offer, before it asks the person, so
/// a terminal that takes no part in file transfers is noticed after a few seconds. After that the
/// person may take a while to decide, and a big file takes a while to cross.
const WAITS: Waits = Waits {
    first: Duration::from_secs(3),
    next: Duration::from_secs(60),
};

pub fn run(command: Command, context: &Context, opener: &impl Opener) -> Result<String, CliError> {
    match command {
        Command::Help => Ok(String::new()),
        Command::Save { files } => save(&files, context),
        Command::Get { name } => get(name, context),
        Command::View { file } => match plan(file, context.at) {
            ViewPlan::OpenHere(path) => {
                std::fs::metadata(&path).map_err(|source| TransferError::Io {
                    path: path.clone(),
                    source,
                })?;
                opener.open(&path)?;
                Ok(format!("Opened {}", path.display()))
            }
            ViewPlan::SendToShelf(path) => {
                save(std::slice::from_ref(&path), context)?;
                Ok(VIEW_SENT.to_owned())
            }
        },
    }
}

fn transfer_id(context: &Context, word: &str) -> Result<KittyId, CliError> {
    KittyId::parse(&format!("temor-{word}-{}", context.pid))
        .map_err(|err| CliError::Transfer(err.into()))
}

fn save(files: &[PathBuf], context: &Context) -> Result<String, CliError> {
    let id = transfer_id(context, "save")?;
    let mut tty = open_tty()?;
    drive_send(&mut tty, id, files, limits(), WAITS)?;
    Ok(saved_words(files.len()))
}

fn get(name: Option<String>, context: &Context) -> Result<String, CliError> {
    let id = transfer_id(context, "get")?;
    let folder = std::env::current_dir().map_err(CliError::Folder)?;
    let root = Root::open(&folder)?;
    let request = Message {
        name,
        ..Message::new(Action::Receive, id)
    };
    let mut tty = open_tty()?;
    let saved = drive_receive(&mut tty, &root, &request, limits(), WAITS)?;
    let names: Vec<String> = saved
        .iter()
        .map(|file| file.path.leaf().to_owned())
        .collect();
    Ok(got_words(&names))
}
