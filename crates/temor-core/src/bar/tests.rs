use super::{
    BarAction, BarContext, BarIn, BarKey, BarOut, BarState, Candidate, OpenBar, PathResolver,
    PluginRow, RowKey, Target, bar_step,
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

fn highlighted(state: &BarState) -> Option<&str> {
    match state {
        BarState::Open(bar) => bar.highlighted.as_ref().map(|key| key.0.as_str()),
        _ => None,
    }
}

#[test]
fn toggle_opens_and_closes_and_escape_closes() {
    let (open, out) = step(BarState::Closed, BarIn::Toggle);
    assert!(matches!(open, BarState::Open(_)));
    assert_eq!(out, vec![BarOut::Refreshed]);
    assert_eq!(
        step(open.clone(), BarIn::Toggle),
        (BarState::Closed, vec![BarOut::Closed])
    );
    assert_eq!(
        step(open, BarIn::Key(BarKey::Escape)),
        (BarState::Closed, vec![BarOut::Closed])
    );
    assert_eq!(
        step(BarState::Closed, BarIn::Key(BarKey::Escape)),
        (BarState::Closed, vec![])
    );
}

#[test]
fn typing_reranks_asks_providers_and_highlights_the_top_row() {
    let (state, _) = step(BarState::Closed, BarIn::Toggle);
    let (state, out) = step(state, BarIn::Typed("logs 2".into()));
    assert_eq!(
        out,
        vec![BarOut::AskProviders("logs 2".into()), BarOut::Refreshed]
    );
    assert_eq!(highlighted(&state), Some("tab:2"));
}

#[test]
fn selection_stops_at_the_ends() {
    let start = BarState::Open(open_with("logs"));
    #[rustfmt::skip]
    let cases: &[(&str, &[BarKey], &str)] = &[
        ("down moves on",                &[BarKey::Down], "tab:2"),
        ("tab moves on",                 &[BarKey::Tab, BarKey::Tab], "tab:3"),
        ("down stops at the last",       &[BarKey::Down, BarKey::Down, BarKey::Down, BarKey::Down], "tab:3"),
        ("up stops at the first",        &[BarKey::Up], "tab:1"),
        ("shift-tab moves back",         &[BarKey::Down, BarKey::BackTab], "tab:1"),
    ];
    for (name, keys, want) in cases {
        let state = keys
            .iter()
            .fold(start.clone(), |state, key| step(state, BarIn::Key(*key)).0);
        assert_eq!(highlighted(&state), Some(*want), "{name}");
    }
}

#[test]
fn enter_and_click_pick_a_row() {
    let start = BarState::Open(open_with("logs"));
    let (state, _) = step(start.clone(), BarIn::Key(BarKey::Down));
    let (picked, out) = step(state, BarIn::Key(BarKey::Enter));
    let BarState::Picked(row) = &picked else {
        panic!("{picked:?}")
    };
    assert_eq!(row.action, BarAction::Go(Target::Tab(TabId(2))));
    assert_eq!(out, vec![BarOut::Run(row.clone())]);
    let (clicked, _) = step(start.clone(), BarIn::Click(RowKey("tab:3".into())));
    assert!(matches!(clicked, BarState::Picked(ref row) if row.key == RowKey("tab:3".into())));
    let (missed, out) = step(start.clone(), BarIn::Click(RowKey("tab:9".into())));
    assert_eq!((missed, out), (start, vec![]));
}

#[test]
fn provider_rows_for_an_old_query_are_dropped_and_the_highlight_survives() {
    let plugin = PluginRow {
        plugin: PluginId("git".into()),
        item: "x".into(),
        title: "logs branch".into(),
    };
    let start = step(BarState::Open(open_with("logs")), BarIn::Key(BarKey::Down)).0;
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
    assert_eq!(highlighted(&state), Some("tab:2"));
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
fn typing_resets_the_highlight_to_the_new_top_row() {
    let cx = BarContext {
        candidates: &[],
        resolver: &Xclip,
        ai: AiSwitch::On,
    };
    let (state, _) = bar_step(BarState::Closed, BarIn::Toggle, cx);
    let (state, _) = bar_step(state, BarIn::Typed("x".into()), cx);
    assert_eq!(highlighted(&state), Some("ai:suggest"));
    let (state, _) = bar_step(state, BarIn::Typed("xclip -o".into()), cx);
    assert_eq!(
        highlighted(&state),
        Some("run"),
        "the Run row on top takes the highlight"
    );
    let (_, out) = bar_step(state, BarIn::Key(BarKey::Enter), cx);
    assert!(
        matches!(out.as_slice(), [BarOut::Run(row)] if matches!(row.action, BarAction::Run(_)))
    );
}
