//! Frecency scores and the providers' candidates, ranked.

use super::{DirectoryUse, PathResolver, Sources, SpaceEntry, UnixSecs, Visits, candidates, rank};
use crate::bar::{BarAction, LayoutName, RowGroup, Target};
use crate::space::{Folder, Pin, Tab};
use crate::split::SplitTree;
use crate::vocab::ai::AiSwitch;
use crate::vocab::ids::{FolderId, PaneId, TabId};
use crate::vocab::text::{CommandText, Cwd, HostAlias, TabTitle};
use ds_style::space::list::SpaceId;
use temor_pane::keys::Action;

const DAY: u64 = 24 * 60 * 60;
const NOW: u64 = 1_000_000_000;

struct NoPath;
impl PathResolver for NoPath {
    fn resolves(&self, _: &str) -> bool {
        false
    }
}

fn ago(count: u32, seconds: u64) -> Visits {
    Visits {
        count,
        last: UnixSecs(NOW - seconds),
    }
}

#[test]
fn frecency_is_the_count_times_the_weight_of_the_last_visit_band() {
    #[rustfmt::skip]
    let cases: &[(&str, Visits, u32)] = &[
        ("yesterday", ago(3, DAY), 300),
        ("exactly 4 days is still the top band", ago(3, 4 * DAY), 300),
        ("a second past 4 days", ago(3, 4 * DAY + 1), 210),
        ("10 days", ago(3, 10 * DAY), 210),
        ("20 days", ago(3, 20 * DAY), 150),
        ("60 days", ago(3, 60 * DAY), 90),
        ("200 days", ago(3, 200 * DAY), 30),
        ("never opened", ago(0, DAY), 0),
        ("a clock that went back counts as just now", Visits { count: 2, last: UnixSecs(NOW + DAY) }, 200),
        ("a huge count saturates", ago(u32::MAX, DAY), u32::MAX),
    ];
    for (name, visits, want) in cases {
        assert_eq!(visits.frecency(UnixSecs(NOW)).0, *want, "{name}");
    }
}

#[test]
fn a_visit_adds_one_and_moves_the_last_time_forward_only() {
    let seen = ago(2, 10 * DAY).visited(UnixSecs(NOW));
    assert_eq!((seen.count, seen.last), (3, UnixSecs(NOW)));
    let behind = ago(2, 0).visited(UnixSecs(NOW - DAY));
    assert_eq!(
        behind.last,
        UnixSecs(NOW),
        "an older clock reading is ignored"
    );
}

fn tab(id: u64, title: &str) -> Tab {
    Tab {
        id: TabId(id),
        title: TabTitle(title.into()),
        pin: Pin::Loose,
        folder: None,
        tree: SplitTree::single(PaneId(id)),
    }
}

#[test]
fn providers_rank_by_section_then_by_frecency_and_drop_what_cannot_be_matched() {
    let spaces = [
        SpaceEntry {
            id: SpaceId(1),
            name: "Ops".into(),
        },
        SpaceEntry {
            id: SpaceId(2),
            name: "  ".into(),
        },
    ];
    let tabs = [tab(7, "src build")];
    let folders = [Folder {
        id: FolderId(2),
        title: TabTitle("src notes".into()),
    }];
    let hosts = [HostAlias("src-box".into())];
    let layouts = [LayoutName("src layout".into())];
    let recent = [
        CommandText("cd src".into()),
        CommandText("echo src\u{7}".into()),
    ];
    let directories = [
        DirectoryUse {
            cwd: Cwd("~/src/old".into()),
            visits: ago(9, 200 * DAY),
        },
        DirectoryUse {
            cwd: Cwd("~/src/new".into()),
            visits: ago(2, DAY),
        },
    ];
    let sources = Sources {
        actions: &[Action::SplitRight],
        spaces: &spaces,
        tabs: &tabs,
        folders: &folders,
        hosts: &hosts,
        layouts: &layouts,
        recent: &recent,
        directories: &directories,
        now: UnixSecs(NOW),
    };
    let all = candidates(&sources);
    assert_eq!(
        all.len(),
        9,
        "an empty name and an untypable command are left out"
    );
    let rows = rank("src", &all, &[], &NoPath, AiSwitch::Off);
    let keys: Vec<&str> = rows.iter().map(|r| r.key.0.as_str()).collect();
    assert_eq!(
        keys,
        vec![
            "tab:7",
            "folder:2",
            "layout:src layout",
            "host:src-box",
            "recent:cd src",
            "dir:~/src/new",
            "dir:~/src/old",
        ]
    );
    assert!(rows.iter().all(|r| r.group == RowGroup::Match));
    let ops = rank("ops", &all, &[], &NoPath, AiSwitch::Off);
    assert_eq!(ops.len(), 1);
    assert_eq!(ops[0].action, BarAction::Go(Target::Space(SpaceId(1))));
}
