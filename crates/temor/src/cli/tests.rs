//! Tables over the pure parts: reading the command line, the plain-language messages and the
//! view decision. The transfers themselves are tested in temor-transfer.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

use temor_transfer::TransferError;

use super::args::{Command, UsageError, parse};
use super::text::{got_words, saved_words, transfer_words};
use super::view::{ViewPlan, Where, plan, where_from};

fn argv(words: &[&str]) -> Vec<OsString> {
    std::iter::once("temor")
        .chain(words.iter().copied())
        .map(OsString::from)
        .collect()
}

fn files(names: &[&str]) -> Vec<PathBuf> {
    names.iter().map(PathBuf::from).collect()
}

#[test]
fn the_command_line_is_read() {
    use Command::{Get, Help, Save, View};
    #[rustfmt::skip]
    #[allow(clippy::type_complexity)] // a table of named rows
    let cases: Vec<(&str, Vec<&str>, Option<Result<Command, UsageError>>)> = vec![
        ("no words start the window", vec![], None),
        ("another word starts the window", vec!["--temor-session-leader"], None),
        ("a file name alone is not ours", vec!["report.pdf"], None),
        ("save one", vec!["save", "a.txt"], Some(Ok(Save { files: files(&["a.txt"]) }))),
        ("save several", vec!["save", "a.txt", "b c.txt"], Some(Ok(Save { files: files(&["a.txt", "b c.txt"]) }))),
        ("save needs a file", vec!["save"], Some(Err(UsageError::NothingToSave))),
        ("dashes end the options", vec!["save", "--", "-odd"], Some(Ok(Save { files: files(&["-odd"]) }))),
        ("an unknown option", vec!["save", "--fast", "a"], Some(Err(UsageError::UnknownOption("--fast".into())))),
        ("get the newest", vec!["get"], Some(Ok(Get { name: None }))),
        ("get by name", vec!["get", "a.txt"], Some(Ok(Get { name: Some("a.txt".into()) }))),
        ("get one at a time", vec!["get", "a", "b"], Some(Err(UsageError::OneFileToGet))),
        ("view one", vec!["view", "a.png"], Some(Ok(View { file: PathBuf::from("a.png") }))),
        ("view needs a file", vec!["view"], Some(Err(UsageError::NothingToView))),
        ("view one at a time", vec!["view", "a", "b"], Some(Err(UsageError::OneFileToView))),
        ("help", vec!["get", "--help"], Some(Ok(Help))),
        ("short help", vec!["view", "-h"], Some(Ok(Help))),
    ];
    for (name, words, want) in cases {
        assert_eq!(parse(&argv(&words)), want, "{name}");
    }
}

#[test]
fn errors_and_results_read_as_plain_sentences() {
    let io_error = |kind| TransferError::Io {
        path: PathBuf::from("a.txt"),
        source: io::Error::from(kind),
    };
    #[rustfmt::skip]
    let cases = vec![
        ("no answer", TransferError::NoAnswer,
            "Nothing answered. This works only inside a temor window, and that window has to be the one you are typing in"),
        ("the person said no", TransferError::Declined { message: "User refused the transfer".into() },
            "You said no in temor, so nothing was sent"),
        ("another reason", TransferError::Declined { message: "there is nothing on the shelf".into() },
            "temor couldn't finish: there is nothing on the shelf"),
        ("a missing file", io_error(io::ErrorKind::NotFound), "Couldn't use a.txt: there is no such file"),
        ("a locked file", io_error(io::ErrorKind::PermissionDenied), "Couldn't use a.txt: you don't have permission"),
        ("a folder", TransferError::NotAFile { path: PathBuf::from("src") },
            "src is not an ordinary file. Pick a file, not a folder"),
        ("a broken line", TransferError::Link(io::Error::from(io::ErrorKind::BrokenPipe)),
            "The connection to the terminal broke"),
    ];
    for (name, err, want) in cases {
        assert_eq!(transfer_words(&err), want, "{name}");
    }
    assert_eq!(saved_words(1), "Sent 1 file to your shelf");
    assert_eq!(saved_words(3), "Sent 3 files to your shelf");
    assert_eq!(
        got_words(&["a.txt".to_owned()]),
        "Saved a.txt in this folder"
    );
    assert_eq!(got_words(&[]), "Saved 0 files in this folder");
}

#[test]
fn viewing_over_ssh_goes_through_the_shelf() {
    use std::ffi::OsStr;
    let file = || PathBuf::from("a.png");
    assert_eq!(plan(file(), Where::Here), ViewPlan::OpenHere(file()));
    assert_eq!(plan(file(), Where::OverSsh), ViewPlan::SendToShelf(file()));
    let some = Some(OsStr::new("10.0.0.1 22 10.0.0.2 22"));
    assert_eq!(where_from(None, None), Where::Here);
    assert_eq!(where_from(some, None), Where::OverSsh);
    assert_eq!(
        where_from(None, Some(OsStr::new("/dev/pts/3"))),
        Where::OverSsh
    );
}
