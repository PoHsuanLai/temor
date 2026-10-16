//! `row.facts` through a real plugin.

use temor_plugins::message::Fact;
use temor_plugins::{FOLDER_TABLE, Launcher, PluginError, Waits};

use crate::support::{ALL, Host, launcher, quick};

#[test]
fn the_facts_a_plugin_gives_are_cleaned_and_a_failing_plugin_gives_none() {
    let host = Host::new();
    host.install("fine", &[], ALL);
    let facts = launcher().facts(&host.plugin("fine"), FOLDER_TABLE, "/home/me/app");
    assert_eq!(
        facts,
        Ok(vec![Fact {
            label: "cluster".to_owned(),
            value: "prod".to_owned()
        }])
    );

    // fault name, waits, the error must say the plugin gave no answer
    let hello_fast = Waits {
        hello: std::time::Duration::from_millis(300),
        ..quick()
    };
    let faults: &[(&str, Waits)] = &[
        ("crash-on-request", quick()),
        ("hang-on-request", quick()),
        ("garbage", quick()),
        ("mute", hello_fast),
        ("other-version", quick()),
        ("provides-nothing", quick()),
    ];
    for (fault, waits) in faults {
        let id = format!("f-{fault}");
        host.install(&id, &["--fault", *fault], ALL);
        let launcher = Launcher::new(*waits, |_| {});
        let got = launcher.facts(&host.plugin(&id), FOLDER_TABLE, "/x");
        assert!(
            matches!(got, Err(PluginError::Failed { .. })),
            "{fault}: {got:?}"
        );
    }
}

#[test]
fn a_program_that_is_missing_is_set_aside_not_run() {
    let host = Host::new();
    host.write(
        "gone.toml",
        "id = \"gone\"\nname = \"Gone\"\nprotocol = 1\n[program]\npath = \"/nonexistent/plugin\"\n[[provides]]\ncapability = \"row.facts\"\ntables = [\"folder\"]\n",
    );
    let catalog = host.catalog();
    assert!(catalog.plugins().is_empty());
    assert_eq!(catalog.problems().len(), 1);
}
