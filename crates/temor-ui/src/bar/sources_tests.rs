use std::collections::BTreeMap;

use temor_client::model::bar::{Target, UnixSecs, Visits};
use temor_client::model::space::{Pin, Tab};
use temor_client::model::split::SplitTree;
use temor_client::model::vocab::ids::{PaneId, Seq, TabId};
use temor_client::model::vocab::text::{CommandText, Cwd, TabTitle};
use temor_client::model::workspace::{ClosedIn, ClosedState, KeepClosed, closed_step};

use super::{closed_candidate, directory_uses, recent_commands};

fn closed_with(titles: &[&str]) -> ClosedState {
    titles.iter().enumerate().fold(
        ClosedState::new(KeepClosed::DEFAULT),
        |state, (n, title)| {
            let tab = Tab {
                id: TabId(n as u64 + 1),
                title: TabTitle((*title).to_owned()),
                pin: Pin::Loose,
                folder: None,
                tree: SplitTree::single(PaneId(n as u64 + 1)),
            };
            closed_step(
                state,
                ClosedIn::Push {
                    tab,
                    focus: PaneId(n as u64 + 1),
                },
            )
            .0
        },
    )
}

#[test]
fn only_the_newest_closed_tab_is_offered() {
    assert_eq!(closed_candidate(&closed_with(&[])), None);
    let candidate = closed_candidate(&closed_with(&["build", "logs"])).expect("a closed tab");
    assert_eq!(candidate.title, "Reopen logs");
    assert_eq!(candidate.target, Target::Closed(Seq(1)));
}

#[test]
fn recent_commands_drop_repeats_and_what_cannot_be_typed() {
    let command = |s: &str| CommandText(s.to_owned());
    let got = recent_commands([
        command("make"),
        command("ls\u{1b}[2J"),
        command(""),
        command("make"),
        command("cargo test"),
    ]);
    assert_eq!(got, vec![command("make"), command("cargo test")]);
}

#[test]
fn folders_keep_their_counts_and_panes_add_one_visit() {
    let now = UnixSecs(1000);
    let mut counted = BTreeMap::new();
    counted.insert(
        "/src/a".to_owned(),
        Visits {
            count: 5,
            last: UnixSecs(10),
        },
    );
    let here = [Cwd("/src/a".to_owned()), Cwd("/src/b".to_owned())];
    let uses = directory_uses(&counted, &here, now);
    let got: Vec<(&str, u32)> = uses
        .iter()
        .map(|u| (u.cwd.as_str(), u.visits.count))
        .collect();
    assert_eq!(got, [("/src/a", 5), ("/src/b", 1)]);
}
