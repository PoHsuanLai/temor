use super::*;
use crate::transfer::Origin;
use ds_core::time::stamp::Stamp;
use temor_core::vocab::ids::{Bytes, DownloadId, TabId};
use temor_core::vocab::text::{HostName, ShelfPath};
use temor_testkit::run_steps;

fn shelf() -> Shelf {
    Shelf::new(ShelfPath("/home/a/Downloads".into()))
}

fn incoming(name: &str, size: Option<u64>, origin: Origin) -> ShelfIn {
    ShelfIn::Incoming(Incoming {
        source: Source::Kitty,
        origin,
        name: name.into(),
        size: size.map(Bytes),
        at: Stamp(1),
    })
}

fn local() -> Origin {
    Origin::Local {
        tab: TabId(1),
        block: None,
    }
}

fn run(start: Shelf, inputs: Vec<ShelfIn>) -> (Shelf, Vec<ShelfOut>) {
    run_steps(start, inputs, shelf_step)
}

#[test]
fn names_are_made_safe_and_numbered() {
    #[rustfmt::skip]
    let safe: Vec<(&str, &str, &str)> = vec![
        ("a path keeps its last part", "../../etc/passwd", "passwd"),
        ("windows separators too", "C:\\tmp\\a.txt", "a.txt"),
        ("hidden names lose their dots", ".bashrc", "bashrc"),
        ("empty becomes download", "", "download"),
        ("control characters go", "a\u{7}b\n.txt", "ab.txt"),
        ("only dots", "...", "download"),
    ];
    for (name, raw, want) in safe {
        assert_eq!(safe_name(raw), want, "{name}");
    }
    let long = "é".repeat(150);
    assert!(safe_name(&long).len() <= 200 && safe_name(&long).chars().all(|c| c == 'é'));
    #[rustfmt::skip]
    let numbered = [
        ("report.pdf", 0, "report.pdf"),
        ("report.pdf", 1, "report (2).pdf"),
        ("archive.tar.gz", 2, "archive.tar (3).gz"),
        ("README", 1, "README (2)"),
    ];
    for (name, attempt, want) in numbered {
        assert_eq!(candidate_name(name, attempt), want, "{name} {attempt}");
    }
}

#[test]
fn a_file_arrives_fills_and_is_opened() {
    let id = DownloadId(1);
    let (s, out) = run(shelf(), vec![incoming("a.txt", Some(200), local())]);
    assert_eq!(
        out,
        vec![
            ShelfOut::Announce(id),
            ShelfOut::Place {
                id,
                path: ShelfPath("/home/a/Downloads/a.txt".into())
            }
        ]
    );
    let view = shelf_view(&s);
    assert_eq!(view.rows[0].state, RowState::Receiving { percent: Some(0) });
    let (s, _) = run(
        s,
        vec![
            ShelfIn::Progress {
                id,
                received: Bytes(50),
            },
            ShelfIn::Opened { id }, // not ready yet: ignored
        ],
    );
    assert_eq!(
        shelf_view(&s).rows[0].state,
        RowState::Receiving { percent: Some(25) }
    );
    assert_eq!(shelf_view(&s).unseen, 0);
    let (s, _) = run(
        s,
        vec![ShelfIn::Done {
            id,
            size: Bytes(200),
        }],
    );
    assert_eq!(shelf_view(&s).unseen, 1);
    let (s, out) = run(s, vec![ShelfIn::Opened { id }]);
    assert_eq!(
        out,
        vec![ShelfOut::Open(ShelfPath("/home/a/Downloads/a.txt".into()))]
    );
    assert_eq!(shelf_view(&s).unseen, 0);
    let (_, out) = run(s, vec![ShelfIn::Revealed { id }]);
    assert_eq!(
        out,
        vec![ShelfOut::Reveal(ShelfPath(
            "/home/a/Downloads/a.txt".into()
        ))]
    );
}

#[test]
fn a_name_in_use_is_never_taken_over() {
    let path = |n: &str| ShelfPath(format!("/home/a/Downloads/{n}"));
    // Two arrivals of one name, then the folder turns out to hold "a (2).txt" already.
    let (s, out) = run(
        shelf(),
        vec![
            incoming("a.txt", None, Origin::Pipe),
            incoming("a.txt", None, Origin::Pipe),
        ],
    );
    let places: Vec<_> = out
        .iter()
        .filter_map(|o| match o {
            ShelfOut::Place { path, .. } => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(places, vec![path("a.txt"), path("a (2).txt")]);
    let (s, out) = run(s, vec![ShelfIn::NameTaken { id: DownloadId(2) }]);
    assert_eq!(
        out,
        vec![ShelfOut::Place {
            id: DownloadId(2),
            path: path("a (3).txt")
        }]
    );
    assert_eq!(s.get(DownloadId(2)).map(|i| i.attempt), Some(2));
    // An unknown id changes nothing.
    let (same, out) = run(s.clone(), vec![ShelfIn::NameTaken { id: DownloadId(9) }]);
    assert_eq!((same, out), (s, vec![]));
}

#[test]
fn failures_and_clearing() {
    let remote = Origin::Remote {
        host: HostName("prod".into()),
        tab: TabId(4),
    };
    let (s, _) = run(
        shelf(),
        vec![
            incoming("a.txt", Some(10), remote),
            incoming("b.txt", None, local()),
            incoming("c.txt", None, Origin::Pipe),
            ShelfIn::Failed {
                id: DownloadId(1),
                why: FailWhy::Broken,
            },
            ShelfIn::Done {
                id: DownloadId(2),
                size: Bytes(3),
            },
            ShelfIn::Done {
                id: DownloadId(1),
                size: Bytes(3),
            }, // after failing: ignored
        ],
    );
    let view = shelf_view(&s);
    let titles: Vec<&str> = view.rows.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(titles, ["c.txt", "b.txt", "a.txt"]);
    assert_eq!(
        view.rows[2].state,
        RowState::Failed {
            message: "The transfer stopped before it finished".into()
        }
    );
    assert_eq!(
        view.rows[2].actions,
        vec![RowAction::ShowInTab(TabId(4)), RowAction::Clear]
    );
    assert_eq!(view.rows[2].detail, "from prod");
    assert_eq!(
        view.rows[1].actions,
        vec![
            RowAction::Open,
            RowAction::Reveal,
            RowAction::UseInPane,
            RowAction::CopyPath,
            RowAction::ShowInTab(TabId(1)),
            RowAction::Clear
        ]
    );
    assert_eq!(view.rows[0].actions, Vec::<RowAction>::new());
    let (finished, _) = run(s.clone(), vec![ShelfIn::Cleared(Clear::Finished)]);
    assert_eq!(finished.items().len(), 1);
    let (one, _) = run(s.clone(), vec![ShelfIn::Cleared(Clear::One(DownloadId(2)))]);
    assert_eq!(one.items().len(), 2);
    let (none, _) = run(s, vec![ShelfIn::Cleared(Clear::All)]);
    assert!(none.items().is_empty());
}

#[test]
fn sizes_read_plainly() {
    let cases = [
        (1, "1 byte"),
        (999, "999 bytes"),
        (1000, "1.0 KB"),
        (12_345_678, "12.3 MB"),
        (2_000_000_000, "2.0 GB"),
    ];
    for (n, want) in cases {
        let (s, _) = run(
            shelf(),
            vec![
                incoming("a", None, Origin::Pipe),
                ShelfIn::Done {
                    id: DownloadId(1),
                    size: Bytes(n),
                },
            ],
        );
        assert_eq!(
            shelf_view(&s).rows[0].detail,
            format!("{want}, from a pipe"),
            "{n}"
        );
    }
}

fn dropped(paths: &[&str], receives_files: bool, site: PaneSite) -> Dropped {
    Dropped {
        paths: paths.iter().map(|p| (*p).to_owned()).collect(),
        receives_files,
        site,
        quoting: Quoting::Posix,
    }
}

#[test]
fn a_drop_is_sent_pasted_or_explained() {
    use DropAction as A;
    use PaneSite::{Local, Remote};
    // Whether the program said it takes dropped files.
    const ANNOUNCED: bool = true;
    const UNKNOWN: bool = false;
    let send = |p: &[&str]| A::Send(p.iter().map(|s| (*s).to_owned()).collect());
    #[rustfmt::skip]
    let cases = vec![
        ("nothing dropped", dropped(&[], ANNOUNCED, Local), A::Nothing),
        ("a program that takes files gets them", dropped(&["/a/b.txt", "/c d.txt"], ANNOUNCED, Local), send(&["/a/b.txt", "/c d.txt"])),
        ("a remote program that takes files too", dropped(&["/a/b.txt"], ANNOUNCED, Remote), send(&["/a/b.txt"])),
        ("a plain local shell gets quoted paths", dropped(&["/a/b c.txt", "/it's.txt"], UNKNOWN, Local),
            A::Paste("'/a/b c.txt' '/it'\\''s.txt' ".into())),
        ("a plain remote shell cannot use a local path", dropped(&["/a/b.txt"], UNKNOWN, Remote),
            A::Tell("This program can't receive dropped files")),
        ("a newline in a name is never pasted", dropped(&["/a\nb"], UNKNOWN, Local),
            A::Tell("That file's name can't be typed into a terminal")),
        ("other names still paste", dropped(&["/a\nb", "/ok"], UNKNOWN, Local), A::Paste("'/ok' ".into())),
    ];
    for (name, input, want) in cases {
        assert_eq!(drop_action(&input), want, "{name}");
    }
}

#[test]
fn quoting_follows_the_shell() {
    #[rustfmt::skip]
    let cases = [
        ("posix plain", Quoting::Posix, "a b", "'a b'"),
        ("posix quote", Quoting::Posix, "it's", "'it'\\''s'"),
        ("posix keeps dollars and globs", Quoting::Posix, "$HOME/*", "'$HOME/*'"),
        ("fish escapes", Quoting::Fish, "a\\b'c", "'a\\\\b\\'c'"),
        ("nu plain", Quoting::Nushell, "a b", "'a b'"),
        ("nu with a quote", Quoting::Nushell, "it's", "`it's`"),
        ("nu with both", Quoting::Nushell, "it's `x`\"", "\"it's `x`\\\"\""),
    ];
    for (name, shell, text, want) in cases {
        assert_eq!(quote(text, shell), want, "{name}");
    }
}
