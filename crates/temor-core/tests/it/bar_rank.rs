//! The ⌘T ranking's fixed rules (§7), one test per rule.

use ds_style::space::list::SpaceId;
use temor_core::bar::{
    BarAction, BarRow, Candidate, Frecency, PathResolver, PluginRow, RowGroup, Section, Target,
    rank,
};
use temor_core::vocab::ai::AiSwitch;
use temor_core::vocab::ids::TabId;
use temor_core::vocab::text::{CommandText, Cwd, HostAlias, PluginId};
use temor_pane::keys::Action;

/// A `PATH` with a fixed set of programs.
struct OnPath(&'static [&'static str]);

impl PathResolver for OnPath {
    fn resolves(&self, program: &str) -> bool {
        self.0.contains(&program)
    }
}

fn candidates() -> Vec<Candidate> {
    let c = |title: &str, target| Candidate {
        title: title.to_string(),
        target,
    };
    vec![
        c(
            "~/src/htop-fork",
            Target::Directory {
                cwd: Cwd("~/src/htop-fork".into()),
                frecency: Frecency(2),
            },
        ),
        c(
            "~/src/top",
            Target::Directory {
                cwd: Cwd("~/src/top".into()),
                frecency: Frecency(9),
            },
        ),
        c("htop", Target::RecentCommand(CommandText("htop".into()))),
        c("Top tab", Target::Tab(TabId(4))),
        c("top", Target::Space(SpaceId(1))),
        c("desktop", Target::Host(HostAlias("desktop".into()))),
    ]
}

fn plugin_rows() -> Vec<PluginRow> {
    vec![PluginRow {
        plugin: PluginId("git".into()),
        name: "Git".into(),
        item: "b1".into(),
        title: "top branch".into(),
    }]
}

fn titles(rows: &[BarRow]) -> Vec<&str> {
    rows.iter().map(|row| row.title.as_str()).collect()
}

fn groups(rows: &[BarRow]) -> Vec<RowGroup> {
    rows.iter().map(|row| row.group).collect()
}

#[test]
fn matches_rank_whole_then_prefix_then_word_start_then_inside_then_by_section() {
    let rows = rank("top", &candidates(), &[], &OnPath(&[]), AiSwitch::Off);
    assert_eq!(
        titles(&rows),
        vec![
            "top",
            "Top tab",
            "~/src/top",
            "~/src/htop-fork",
            "desktop",
            "htop"
        ]
    );
    assert!(rows.iter().all(|row| row.group == RowGroup::Match));
    let none = rank("nothing", &candidates(), &[], &OnPath(&[]), AiSwitch::Off);
    assert!(none.is_empty());
}

#[test]
fn equally_good_folders_rank_by_frecency() {
    let dir = |path: &str, n| Candidate {
        title: path.to_string(),
        target: Target::Directory {
            cwd: Cwd(path.into()),
            frecency: Frecency(n),
        },
    };
    let folders = [dir("~/a/web", 1), dir("~/b/web", 7), dir("~/c/web", 3)];
    let rows = rank("web", &folders, &[], &OnPath(&[]), AiSwitch::Off);
    assert_eq!(titles(&rows), vec!["~/b/web", "~/c/web", "~/a/web"]);
}

#[test]
fn a_run_row_tops_the_list_only_when_the_first_word_is_on_path() {
    let on = rank(
        "top -d 2",
        &candidates(),
        &[],
        &OnPath(&["top"]),
        AiSwitch::Off,
    );
    assert_eq!(on[0].title, "Run `top -d 2` in a new tab");
    assert_eq!(on[0].action, BarAction::Run(CommandText("top -d 2".into())));
    assert_eq!(
        groups(&on).iter().filter(|g| **g == RowGroup::Run).count(),
        1
    );
    let off = rank(
        "top -d 2",
        &candidates(),
        &[],
        &OnPath(&["htop"]),
        AiSwitch::Off,
    );
    assert!(off.iter().all(|row| row.group != RowGroup::Run));
    let blank = rank("   ", &candidates(), &[], &OnPath(&[""]), AiSwitch::Off);
    assert!(
        blank
            .iter()
            .all(|row| !matches!(row.action, BarAction::Run(_))),
        "a blank query is not a command"
    );
    assert_eq!(blank[0].action, BarAction::NewTab, "it is a plain new tab");
}

#[test]
fn plugin_rows_follow_local_matches_in_their_own_order() {
    let rows = rank(
        "top",
        &candidates(),
        &plugin_rows(),
        &OnPath(&["top"]),
        AiSwitch::Off,
    );
    let g = groups(&rows);
    let first_plugin = g.iter().position(|g| *g == RowGroup::Plugin).unwrap();
    assert!(g[..first_plugin].iter().all(|g| *g != RowGroup::Plugin));
    assert_eq!(rows[first_plugin].title, "top branch");
    assert_eq!(first_plugin, rows.len() - 1);
}

#[test]
fn ai_rows_are_always_last_and_absent_when_ai_is_off() {
    let rows = rank(
        "top",
        &candidates(),
        &plugin_rows(),
        &OnPath(&["top"]),
        AiSwitch::On,
    );
    let g = groups(&rows);
    assert_eq!(&g[g.len() - 3..], &[RowGroup::Ai; 3]);
    assert!(g.windows(2).all(|pair| pair[0] <= pair[1]), "{g:?}");
    assert_eq!(
        titles(&rows[rows.len() - 3..]),
        vec![
            "Suggest a command for: top",
            "Ask an agent in a new pane: top",
            "Ask about this tab: top",
        ]
    );
    let none = rank("zzz", &[], &[], &OnPath(&[]), AiSwitch::On);
    assert_eq!(
        groups(&none),
        vec![RowGroup::Ai; 3],
        "AI rows even when nothing else matches"
    );
    let off = rank(
        "top",
        &candidates(),
        &plugin_rows(),
        &OnPath(&["top"]),
        AiSwitch::Off,
    );
    assert!(off.iter().all(|row| row.group != RowGroup::Ai));
    assert_eq!(
        titles(&rank("", &[], &[], &OnPath(&[]), AiSwitch::On)),
        vec!["New tab"],
        "no AI rows for no query, only the plain new tab"
    );
}

#[test]
fn window_actions_are_rows_and_a_sections_rows_sit_together() {
    let c = |title: &str, target| Candidate {
        title: title.to_string(),
        target,
    };
    let all = [
        c("Split right", Target::Action(Action::SplitRight)),
        c("Split down", Target::Action(Action::SplitDown)),
        c("Zoom", Target::Action(Action::ToggleZoom)),
        c("spline work", Target::Tab(TabId(1))),
        c("New tab", Target::Action(Action::NewTab)),
        c("spl-box", Target::Host(HostAlias("spl-box".into()))),
        c("Split view", Target::Tab(TabId(2))),
    ];
    let rows = rank("spl", &all, &[], &OnPath(&[]), AiSwitch::Off);
    assert_eq!(
        titles(&rows),
        vec![
            "Split right",
            "Split down",
            "spline work",
            "Split view",
            "spl-box"
        ]
    );
    let sections: Vec<Section> = rows.iter().map(Section::of).collect();
    assert_eq!(
        sections,
        vec![
            Section::Actions,
            Section::Actions,
            Section::Tabs,
            Section::Tabs,
            Section::Computers
        ]
    );
    // The plain new tab is one row on the empty bar, whichever way it is offered.
    let empty = rank("", &all, &[], &OnPath(&[]), AiSwitch::Off);
    let new_tabs = empty.iter().filter(|r| r.key.0 == "new-tab").count();
    assert_eq!((new_tabs, empty[0].action.clone()), (1, BarAction::NewTab));
}

#[test]
fn each_plugin_gets_its_own_section_after_the_built_in_ones_ordered_by_name() {
    let row = |plugin: &str, name: &str, item: &str| PluginRow {
        plugin: PluginId(plugin.into()),
        name: name.into(),
        item: item.into(),
        title: item.into(),
    };
    // Arrival order interleaves the two plugins; ids sort the other way round from the names.
    let offered = [
        row("a-id", "Zebra", "z1"),
        row("z-id", "Apple", "a1"),
        row("a-id", "Zebra", "z2"),
        row("z-id", "Apple", "a2"),
    ];
    let rows = rank("top", &candidates(), &offered, &OnPath(&[]), AiSwitch::On);
    let tail: Vec<(Section, &str)> = rows
        .iter()
        .skip_while(|row| row.group != RowGroup::Plugin)
        .take_while(|row| row.group == RowGroup::Plugin)
        .map(|row| (Section::of(row), row.title.as_str()))
        .collect();
    assert_eq!(
        tail,
        vec![
            (Section::Plugin(PluginId("z-id".into())), "a1"),
            (Section::Plugin(PluginId("z-id".into())), "a2"),
            (Section::Plugin(PluginId("a-id".into())), "z1"),
            (Section::Plugin(PluginId("a-id".into())), "z2"),
        ]
    );
    assert_eq!(
        Section::of(rows.last().expect("AI rows")),
        Section::Ask,
        "the AI rows stay last"
    );
}
