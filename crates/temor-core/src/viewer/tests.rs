use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use temor_pane::text::{RowText, Wrap};

use super::{MAX_PATHS, candidates, output_paths, resolve, token_at};

fn row(text: &str) -> RowText {
    RowText::plain(text, Wrap::Ends)
}

#[test]
fn the_token_is_the_run_of_non_blank_cells_around_the_click() {
    let line = row("$ cat  src/main.rs:12:5: error");
    let table: [(u16, Option<&str>); 6] = [
        (0, Some("$")),
        (1, None),
        (7, Some("src/main.rs:12:5:")),
        (23, Some("src/main.rs:12:5:")),
        (24, None),
        (99, None),
    ];
    for (col, want) in table {
        assert_eq!(token_at(&line, col).as_deref(), want, "column {col}");
    }
}

#[test]
fn a_wide_characters_spacer_belongs_to_it() {
    let mut line = row("ab");
    line.cells.insert(
        1,
        temor_pane::text::RowCell {
            text: "界".to_owned(),
            span: temor_pane::text::Span::Double,
        },
    );
    line.cells.insert(
        2,
        temor_pane::text::RowCell {
            text: String::new(),
            span: temor_pane::text::Span::Spacer,
        },
    );
    // a界 (spacer) b: one token, whichever of its columns is clicked.
    for col in 0..4 {
        assert_eq!(
            token_at(&line, col).as_deref(),
            Some("a界b"),
            "column {col}"
        );
    }
}

#[test]
fn candidates_peel_what_wraps_and_follows_a_path() {
    let table: [(&str, &[&str]); 7] = [
        ("a.txt", &["a.txt"]),
        ("'a.txt',", &["a.txt"]),
        ("(src/a.rs)", &["src/a.rs"]),
        (
            "src/a.rs:12:5:",
            &["src/a.rs:12:5:", "src/a.rs:12:5", "src/a.rs:12", "src/a.rs"],
        ),
        ("src/a.rs:12:fn", &["src/a.rs:12:fn", "src/a.rs"]),
        ("a:b", &["a:b", "a"]),
        ("''", &[]),
    ];
    for (token, want) in table {
        assert_eq!(candidates(token), want, "{token}");
    }
}

#[test]
fn a_path_resolves_from_the_folder_or_not_at_all() {
    let table: [(Option<&str>, &str, Option<&str>); 6] = [
        (Some("/w"), "a.txt", Some("/w/a.txt")),
        (Some("/w"), "d/../a.txt", Some("/w/d/../a.txt")),
        (Some("/w"), "/etc/hosts", Some("/etc/hosts")),
        (None, "a.txt", None),
        (None, "/etc/hosts", Some("/etc/hosts")),
        (Some("/w"), "a\u{1b}b", None),
    ];
    for (cwd, candidate, want) in table {
        assert_eq!(
            resolve(cwd, candidate),
            want.map(PathBuf::from),
            "{candidate}"
        );
    }
}

fn files(list: &[&str]) -> impl Fn(&Path) -> bool {
    let set: BTreeSet<PathBuf> = list.iter().map(PathBuf::from).collect();
    move |path| set.contains(path)
}

#[test]
fn output_paths_read_what_ls_find_and_rg_print() {
    let on_disk = ["/w/a.txt", "/w/b.txt", "/w/my notes.md", "/w/src/m.rs"];
    let table: [(&str, &[&str]); 7] = [
        // ls in columns
        ("a.txt  b.txt  gone.txt\n", &["/w/a.txt", "/w/b.txt"]),
        // ls -l: the last word is the file
        ("-rw-r--r-- 1 u g 0 Oct 10 a.txt\n", &["/w/a.txt"]),
        // find, a name with a space in it
        (
            "./a.txt\n./my notes.md\n",
            &["/w/./a.txt", "/w/my notes.md"],
        ),
        // rg
        ("src/m.rs:12:fn main() {\nsrc/m.rs:30:}\n", &["/w/src/m.rs"]),
        // nothing printed is a file
        ("total 0\nnope\n", &[]),
        // a file listed twice is one entry
        ("a.txt\na.txt\n", &["/w/a.txt"]),
        // absolute paths need no folder
        ("/w/b.txt\n", &["/w/b.txt"]),
    ];
    for (output, want) in table {
        let got = output_paths(output, Some("/w"), files(&on_disk));
        let want: Vec<PathBuf> = want.iter().map(PathBuf::from).collect();
        assert_eq!(got, want, "{output:?}");
    }
}

#[test]
fn a_relative_listing_with_no_folder_names_nothing() {
    assert!(output_paths("a.txt\n", None, files(&["/w/a.txt"])).is_empty());
}

#[test]
fn a_long_listing_is_cut() {
    let output: String = (0..MAX_PATHS + 50).map(|i| format!("/f/{i}\n")).collect();
    let got = output_paths(&output, None, |_| true);
    assert_eq!(got.len(), MAX_PATHS);
    assert_eq!(got[0], PathBuf::from("/f/0"));
}
