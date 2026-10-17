use super::{
    BarAction, BarContext, BarIn, BarOut, BarState, Candidate, OpenBar, PathResolver, PluginRow,
    RowKey, Target, bar_step,
};
use crate::vocab::ai::AiSwitch;
use crate::vocab::ids::TabId;
use crate::vocab::text::PluginId;

struct Nothing;
impl PathResolver for Nothing {
    fn resolves(&self, _: &str) -> bool {
        false
    }
}

fn candidates() -> Vec<Candidate> {
    (1..=3)
        .map(|n| Candidate {
            title: format!("logs {n}"),
            target: Target::Tab(TabId(n)),
        })
        .collect()
}

fn step(state: BarState, input: BarIn) -> (BarState, Vec<BarOut>) {
    let candidates = candidates();
    let cx = BarContext {
        candidates: &candidates,
        resolver: &Nothing,
        ai: AiSwitch::Off,
    };
    bar_step(state, input, cx)
}

fn open_with(query: &str) -> OpenBar {
    let (state, _) = step(BarState::Closed, BarIn::Toggle);
    let (BarState::Open(bar), _) = step(state, BarIn::Typed(query.into())) else {
        panic!()
    };
    bar
}

fn keys(bar: &OpenBar) -> Vec<&str> {
    bar.rows.iter().map(|row| row.key.0.as_str()).collect()
}

#[test]
fn toggle_opens_and_closes_and_close_closes() {
    let (open, out) = step(BarState::Closed, BarIn::Toggle);
    let BarState::Open(bar) = &open else {
        panic!("{open:?}")
    };
    assert_eq!(out, vec![BarOut::Refreshed]);
    assert_eq!(
        keys(bar)[0],
        "new-tab",
        "an empty bar leads with a plain tab"
    );
    assert_eq!(
        step(open.clone(), BarIn::Toggle),
        (BarState::Closed, vec![BarOut::Closed])
    );
    assert_eq!(
        step(open, BarIn::Close),
        (BarState::Closed, vec![BarOut::Closed])
    );
    assert_eq!(
        step(BarState::Closed, BarIn::Close),
        (BarState::Closed, vec![])
    );
}

#[test]
fn typing_reranks_and_asks_providers() {
    let (state, _) = step(BarState::Closed, BarIn::Toggle);
    let (state, out) = step(state, BarIn::Typed("logs 2".into()));
    assert_eq!(
        out,
        vec![BarOut::AskProviders("logs 2".into()), BarOut::Refreshed]
    );
    let BarState::Open(bar) = state else { panic!() };
    assert_eq!(keys(&bar), ["tab:2"]);
}

#[test]
fn a_picked_row_runs_and_an_unknown_one_changes_nothing() {
    let start = BarState::Open(open_with("logs"));
    let (picked, out) = step(start.clone(), BarIn::Pick(RowKey("tab:2".into())));
    let BarState::Picked(row) = &picked else {
        panic!("{picked:?}")
    };
    assert_eq!(row.action, BarAction::Go(Target::Tab(TabId(2))));
    assert_eq!(out, vec![BarOut::Run(row.clone())]);
    let (missed, out) = step(start.clone(), BarIn::Pick(RowKey("tab:9".into())));
    assert_eq!((missed, out), (start, vec![]));
    let (closed_pick, out) = step(BarState::Closed, BarIn::Pick(RowKey("tab:2".into())));
    assert_eq!((closed_pick, out), (BarState::Closed, vec![]));
}

#[test]
fn provider_rows_for_an_old_query_are_dropped() {
    let plugin = PluginRow {
        plugin: PluginId("git".into()),
        name: "Git".into(),
        item: "x".into(),
        title: "logs branch".into(),
    };
    let start = BarState::Open(open_with("logs"));
    let stale = BarIn::ProviderRows {
        query: "log".into(),
        rows: vec![plugin.clone()],
    };
    assert_eq!(step(start.clone(), stale), (start.clone(), vec![]));
    let fresh = BarIn::ProviderRows {
        query: "logs".into(),
        rows: vec![plugin],
    };
    let (state, out) = step(start, fresh);
    assert_eq!(out, vec![BarOut::Refreshed]);
    let BarState::Open(bar) = state else { panic!() };
    assert_eq!(
        bar.rows.last().map(|row| row.title.as_str()),
        Some("logs branch")
    );
}

struct Xclip;
impl PathResolver for Xclip {
    fn resolves(&self, program: &str) -> bool {
        program == "xclip"
    }
}

#[test]
fn the_run_row_leads_when_the_command_is_on_path_and_ai_rows_follow() {
    let cx = BarContext {
        candidates: &[],
        resolver: &Xclip,
        ai: AiSwitch::On,
    };
    let (state, _) = bar_step(BarState::Closed, BarIn::Toggle, cx);
    let (state, _) = bar_step(state, BarIn::Typed("xclip -o".into()), cx);
    let BarState::Open(bar) = &state else {
        panic!()
    };
    assert_eq!(keys(bar), ["run", "ai:suggest", "ai:agent", "ai:about"]);
    let (_, out) = bar_step(state, BarIn::Pick(RowKey("run".into())), cx);
    assert!(
        matches!(out.as_slice(), [BarOut::Run(row)] if matches!(row.action, BarAction::Run(_)))
    );
}
