//! The hub and a real plugin on a real pipe: `temor-plugins`' example program, installed in a
//! scratch data directory. Needs the example built, which `cargo test --workspace` does. The
//! clipboard and the openers are fakes; the desktop is never touched.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use temor_core::bar::RowKey;
use temor_core::vocab::ids::{BlockId, PaneId};
use temor_core::vocab::text::Cwd;
use temor_pane::PaneEvent;
use temor_pane::report::ReportState;
use temor_plugins::{Catalog, DataDirs, lens_block};
use tokio::sync::mpsc::UnboundedReceiver;

use super::Hub;
use crate::hub::tests::launch;
use crate::plugins::effects::Hands;
use crate::plugins::facts::PluginFact;
use crate::plugins::host::Done;
use crate::plugins::{PluginHost, PluginHub};

const ALL: &str = r#"
[[provides]]
capability = "block.lens"
trigger = "kubectl get"

[[provides]]
capability = "open.provider"
trigger = "gh"

[[provides]]
capability = "agent.adapter"
agent = "claude"

[[provides]]
capability = "row.facts"
tables = ["folder"]
"#;

fn fake_program() -> PathBuf {
    let exe = std::env::current_exe().expect("the test's own path");
    let program = exe
        .parent()
        .and_then(Path::parent)
        .map(|profile| profile.join("examples").join("fake_plugin"))
        .expect("a target directory");
    assert!(
        program.is_file(),
        "{} is missing: run the tests with `cargo test --workspace`, which builds the examples",
        program.display()
    );
    program
}

/// A hub whose plugins are the example program, started with `args`.
fn hub_with(args: &[&str]) -> (Hub, UnboundedReceiver<Done>, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("a scratch directory");
    let folder = root.path().join("user").join("temor").join("plugins");
    std::fs::create_dir_all(&folder).expect("the plugin folder");
    let text = format!(
        "id = \"fake\"\nname = \"Fake\"\nprotocol = 1\n[program]\npath = {:?}\nargs = {:?}\n{ALL}",
        fake_program().display().to_string(),
        args
    );
    std::fs::write(folder.join("fake.toml"), text).expect("the manifest");
    let catalog = Catalog::discover(&DataDirs {
        user: root.path().join("user"),
        system: Vec::new(),
    });
    let mut hub = Hub::new(launch());
    hub.plugins = PluginHub::over(PluginHost::with_catalog(catalog));
    let inbox = hub.plugins.host.take_inbox().expect("the inbox");
    (hub, inbox, root)
}

/// The next finished piece of work, within ten seconds.
fn next(inbox: &mut UnboundedReceiver<Done>) -> Done {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(done) = inbox.try_recv() {
            return done;
        }
        assert!(Instant::now() < deadline, "no plugin work finished in time");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Whatever finishes within a second, if anything does.
fn next_or_quiet(inbox: &mut UnboundedReceiver<Done>) -> Option<Done> {
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        if let Ok(done) = inbox.try_recv() {
            return Some(done);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[derive(Debug, Default)]
struct Spy {
    calls: RefCell<Vec<String>>,
}

impl Hands for Spy {
    fn copy(&self, text: &str) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("copy {text}"));
        Ok(())
    }
    fn open_url(&self, url: &str) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("url {url}"));
        Ok(())
    }
    fn open_file(&self, path: &str) -> Result<(), String> {
        self.calls.borrow_mut().push(format!("file {path}"));
        Ok(())
    }
}

#[test]
fn a_plugins_facts_join_the_pane_and_a_failing_plugin_adds_nothing() {
    let pane = PaneId(1);
    // fault arguments, the facts the pane ends with
    type Row<'a> = (&'a str, Vec<&'a str>, Vec<(&'a str, &'a str)>);
    let table: Vec<Row> = vec![
        ("a plugin that answers", vec![], vec![("cluster", "prod")]),
        (
            "a plugin that crashes",
            vec!["--fault", "crash-on-request"],
            vec![],
        ),
    ];
    for (why, args, want) in table {
        let (mut hub, mut inbox, _root) = hub_with(&args);
        hub.plugin_facts_start(pane, "/work");
        // A second start while the first is reading does nothing.
        hub.plugin_facts_start(pane, "/work");
        let done = next(&mut inbox);
        hub.plugin_done(done);
        let facts: Vec<(String, String)> = hub
            .plugin_facts_of(pane)
            .into_iter()
            .map(|f: PluginFact| (f.label, f.value))
            .collect();
        let want: Vec<(String, String)> = want
            .into_iter()
            .map(|(l, v)| (l.to_owned(), v.to_owned()))
            .collect();
        assert_eq!(facts, want, "{why}");
        assert!(inbox.try_recv().is_err(), "{why}: one read, one answer");
        assert_eq!(
            hub.plugin_facts_summary(pane),
            want.first().map(|(l, v)| format!("{l} {v}")),
            "{why}"
        );
    }
}

#[test]
fn the_bar_lists_what_a_provider_offered_for_the_text_and_choosing_a_row_copies() {
    let (mut hub, mut inbox, _root) = hub_with(&[]);
    let spy = Rc::new(Spy::default());
    hub.plugins.hands = Some(spy.clone());

    hub.bar_toggle();
    hub.bar_typed("gh pr".to_owned());
    hub.plugin_done(next(&mut inbox));
    let data = hub.bar_data().expect("the bar is open");
    let rows: Vec<(&str, &str)> = data
        .rows
        .iter()
        .map(|row| (row.key.0.as_str(), row.title.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("plugin:fake:high", "pr high"),
            ("plugin:fake:low", "pr low")
        ]
    );

    hub.bar_pick(&RowKey("plugin:fake:high".to_owned()));
    hub.plugin_done(next(&mut inbox));
    assert_eq!(*spy.calls.borrow(), ["copy high"]);
}

#[test]
fn rows_for_text_that_was_typed_over_are_dropped() {
    let (mut hub, mut inbox, _root) = hub_with(&[]);
    hub.bar_toggle();
    hub.bar_typed("gh a".to_owned());
    let old = next(&mut inbox);
    hub.bar_typed("gh ab".to_owned());
    hub.plugin_done(old);
    assert!(hub.bar_data().expect("open").rows.is_empty());
    hub.plugin_done(next(&mut inbox));
    assert_eq!(hub.bar_data().expect("open").rows.len(), 2);
}

#[test]
fn a_lens_draws_a_block_and_a_picture_nobody_asked_for_is_dropped() {
    let (mut hub, mut inbox, _root) = hub_with(&[]);
    let (pane, block) = (PaneId(1), BlockId(4));
    let lenses = hub.plugins.host.catalog.lenses("kubectl get pods");
    let plugin = lenses.into_iter().next().expect("a lens for kubectl get");
    hub.lens_request(
        pane,
        block,
        plugin.clone(),
        lens_block("kubectl get pods", "NAME", "/work", 640),
    );
    hub.plugin_done(next(&mut inbox));
    let image = hub.lens_image_of(pane, block).expect("the picture");
    assert_eq!(
        (
            image.plugin.as_str(),
            image.drawing.width(),
            image.drawing.height()
        ),
        ("fake", 2, 1)
    );
    hub.lens_hide(pane, block);
    assert!(hub.lens_image_of(pane, block).is_none());

    hub.lens_request(pane, block, plugin, lens_block("bad", "", "/work", 640));
    // A picture of the wrong size is refused on the way in: nothing arrives, nothing is stored.
    assert!(next_or_quiet(&mut inbox).is_none());
    assert!(hub.lens_image_of(pane, block).is_none());
}

#[test]
fn an_adapter_reports_the_agents_states_as_the_panes_status() {
    let (mut hub, mut inbox, _root) = hub_with(&[]);
    let pane = PaneId(1);
    hub.book.ws.cwds.insert(pane, Cwd("/finish".to_owned()));
    // What the adapter sends, as it sends it: the pane has no session to hear it.
    let (sent, mut events) = tokio::sync::mpsc::unbounded_channel();
    hub.events = sent;
    hub.plugin_adapter_start(pane, "claude");
    let ended = next(&mut inbox);
    assert!(matches!(ended, Done::AdapterEnded { pane: p, .. } if p == pane));
    hub.plugin_done(ended);
    assert!(hub.plugins.adapters.is_empty());

    let mut seen = Vec::new();
    while let Ok((owner, event)) = events.try_recv() {
        assert_eq!(owner, pane);
        if let PaneEvent::Reported(line) = event {
            seen.push((line.state, line.note, line.app));
        }
    }
    assert_eq!(
        seen,
        vec![
            (
                ReportState::Working,
                Some("thinking".to_owned()),
                Some("claude".to_owned())
            ),
            (
                ReportState::Done,
                Some("all done".to_owned()),
                Some("claude".to_owned())
            ),
        ]
    );
}

#[test]
fn an_adapter_follows_the_agent_and_stops_when_it_leaves() {
    let (mut hub, _inbox, _root) = hub_with(&[]);
    let pane = PaneId(1);
    hub.book.ws.cwds.insert(pane, Cwd("/work".to_owned()));
    // was, is, adapters running after
    let steps: Vec<(Option<&str>, Option<&str>, usize)> = vec![
        (None, Some("codex"), 0),
        (None, Some("claude"), 1),
        (Some("claude"), Some("claude"), 1),
        (Some("claude"), None, 0),
    ];
    for (was, is, running) in steps {
        hub.plugin_adapter_follows(pane, was.map(str::to_owned), is.map(str::to_owned));
        assert_eq!(hub.plugins.adapters.len(), running, "{was:?} to {is:?}");
    }
}
