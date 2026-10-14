//! Stored and wire types: their serde forms (CONVENTIONS §12), round trips, and the Spaces kit's
//! flat payload.

use ds_core::testing::word_matches_serde;
use ds_core::time::stamp::Stamp;
use ds_style::space::list::{SpaceId, Spaces};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use temor_core::block::{Author, Block, Taint};
use temor_core::pane::{CliGate, PaneKind, TerminalPane, TerminalRole};
use temor_core::presence::{AgentState, Seen, StatusReport, StatusSource, WaitKind};
use temor_core::proposal::{
    ChangeKind, CharClass, FileChange, FileChanges, Proposal, Sanitised, TrialVerdict,
    UncheckedWhy, sanitise,
};
use temor_core::remote::{Grant, Host, HostConsent, RemoteSession, RemoteTier};
use temor_core::space::{
    AccountRef, Credential, Pin, Place, Profile, Project, Route, Tab, TemorSpaces,
};
use temor_core::split::{Axis, Direction, SplitTree};
use temor_core::transfer::{Download, Origin, Transfer};
use temor_core::vocab::ai::AiSwitch;
use temor_core::vocab::ids::*;
use temor_core::vocab::text::*;

fn round_trips<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: &T) -> Value {
    let json = serde_json::to_value(value).unwrap();
    let back: T = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(&back, value, "{json}");
    json
}

fn tree() -> SplitTree {
    SplitTree::single(PaneId(1))
        .split(PaneId(1), PaneId(2), Direction::Down)
        .unwrap()
}

fn project() -> Project {
    Project {
        root: Some(ProjectRoot("/home/me/src/temor".into())),
        profile: Profile {
            env: vec![(EnvKey::new("RUST_LOG").unwrap(), EnvValue("debug".into()))],
            ssh_identity: Some(KeyPath("~/.ssh/work".into())),
            dotenv: Some(RelPath(".env".into())),
            credentials: vec![Credential {
                env: EnvKey::new("ANTHROPIC_API_KEY").unwrap(),
                account: AccountRef::new("anthropic-work").unwrap(),
            }],
        },
        routes: vec![Route {
            pattern: RoutePattern("ssh prod-*".into()),
        }],
    }
}

fn place() -> Place {
    Place {
        tabs: vec![Tab {
            id: TabId(3),
            title: TabTitle("build".into()),
            pin: Pin::Pinned {
                relaunch: Relaunch("cargo watch".into()),
            },
            folder: Some(FolderId(1)),
            tree: tree(),
        }],
        folders: vec![],
        focused: Some(TabId(3)),
        viewers: vec![],
        cwds: vec![],
    }
}

#[test]
fn closed_sets_serialise_as_their_slugs() {
    word_matches_serde::<Axis>();
    word_matches_serde::<Direction>();
    word_matches_serde::<CliGate>();
    word_matches_serde::<WaitKind>();
    word_matches_serde::<Seen>();
    word_matches_serde::<Taint>();
    word_matches_serde::<UncheckedWhy>();
    word_matches_serde::<ChangeKind>();
    word_matches_serde::<CharClass>();
    word_matches_serde::<Grant>();
    word_matches_serde::<AiSwitch>();
}

#[test]
fn enums_with_data_are_adjacently_tagged() {
    #[rustfmt::skip]
    let cases: Vec<(&str, Value, Value)> = vec![
        ("pin", serde_json::to_value(Pin::Pinned { relaunch: Relaunch("top".into()) }).unwrap(),
            json!({"kind": "pinned", "v": {"relaunch": "top"}})),
        ("loose pin", serde_json::to_value(Pin::Loose).unwrap(), json!({"kind": "loose"})),
        ("waiting", serde_json::to_value(AgentState::Waiting(WaitKind::SignIn)).unwrap(),
            json!({"kind": "waiting", "v": "sign_in"})),
        ("adapter", serde_json::to_value(StatusSource::Adapter(PluginId("cc".into()))).unwrap(),
            json!({"kind": "adapter", "v": "cc"})),
        ("unchecked", serde_json::to_value(TrialVerdict::Unchecked(UncheckedWhy::TimedOut)).unwrap(),
            json!({"kind": "unchecked", "v": "timed_out"})),
        ("changed", serde_json::to_value(Sanitised::Changed { removed: vec![CharClass::Bidi] }).unwrap(),
            json!({"kind": "changed", "v": {"removed": ["bidi"]}})),
        ("tier", serde_json::to_value(RemoteTier::Plain).unwrap(), json!({"kind": "plain"})),
    ];
    for (name, got, want) in cases {
        assert_eq!(got, want, "{name}");
    }
}

#[test]
fn every_stored_type_round_trips() {
    round_trips(&project());
    round_trips(&place());
    round_trips(&tree().zoom(PaneId(2)).unwrap());
    round_trips(&PaneKind::Terminal(TerminalPane {
        role: TerminalRole::AgentCli {
            program: AgentLabel("claude".into()),
            gate: CliGate::HookBridged,
        },
        cwd: Some(Cwd("/tmp".into())),
        host: None,
    }));
    round_trips(&PaneKind::AgentView(AgentSessionRef(4)));
    let span = GridSpan {
        first: GridRow(1),
        last: GridRow(4),
    };
    round_trips(&Block {
        id: BlockId(9),
        prompt: span,
        command: CommandText("make".into()),
        output: span,
        exit: Some(ExitCode(2)),
        by: Author::Proposal {
            turn: TurnRef(3),
            taint: Taint::ReadUntrusted,
        },
        started: Stamp(10),
        ended: Some(Stamp(20)),
    });
    round_trips(&StatusReport {
        pane: PaneId(1),
        state: AgentState::Done(Seen::Unseen),
        seq: Seq(5),
        source: StatusSource::ProgramStatus,
        note: Some(Note("tests pass".into())),
    });
    let (text, _) = sanitise("cargo test");
    for verdict in [
        TrialVerdict::Clean {
            changes: FileChanges {
                files: vec![FileChange {
                    path: RelPath("a".into()),
                    how: ChangeKind::Created,
                }],
            },
        },
        TrialVerdict::Failed {
            exit: ExitCode(1),
            first_error: ErrorLine("no such file".into()),
        },
        TrialVerdict::Unchecked(UncheckedWhy::Remote),
    ] {
        round_trips(&Proposal {
            text: text.clone(),
            turn: TurnRef(1),
            taint: Taint::Clean,
            verdict,
        });
    }
    round_trips(&Host {
        alias: HostAlias("prod".into()),
        target: SshTarget("me@prod:22".into()),
        space_pin: Some(SpaceId(2)),
        consent: HostConsent {
            clipboard: Grant::Refused,
            transfers: Grant::Allowed,
            ..HostConsent::default()
        },
        tier: RemoteTier::Helper {
            version: HelperHash("abc".into()),
        },
    });
    round_trips(&RemoteSession {
        host: HostAlias("prod".into()),
        id: RemoteSessionId(1),
        cwd: RemotePath("/srv".into()),
    });
    for (from, state) in [
        (
            Origin::Remote {
                host: HostName("prod".into()),
                tab: TabId(1),
            },
            Transfer::Receiving {
                received: Bytes(10),
            },
        ),
        (
            Origin::Local {
                tab: TabId(1),
                block: Some(BlockId(2)),
            },
            Transfer::Saved,
        ),
        (Origin::Pipe, Transfer::Broken),
        (Origin::LocalStream { tab: TabId(2) }, Transfer::Offered),
    ] {
        round_trips(&Download {
            id: DownloadId(1),
            path: ShelfPath("a.png".into()),
            from,
            size: Bytes(10),
            at: Stamp(1),
            state,
        });
    }
}

#[test]
fn a_project_sits_flat_beside_the_look_in_the_spaces_file() {
    let mut spaces: TemorSpaces = Spaces::first_run([project()], Project::default);
    let id = spaces.current().id;
    spaces.edit_recall(id, |recalled| *recalled = place());
    let json = round_trips(&spaces);
    let space = &json["spaces"][0];
    for key in ["root", "profile", "routes"] {
        assert!(space.get(key).is_some(), "{key} is written flat: {space}");
    }
    for kit_key in ["id", "name", "dots", "grain", "theme"] {
        assert!(
            space.get(kit_key).is_some(),
            "the kit still writes {kit_key}"
        );
    }
    assert_eq!(spaces.recall().of(id), place());
}

#[test]
fn stored_forms_are_read_leniently() {
    let file = json!({
        "spaces": [{
            "id": 0, "name": "Work", "a_key_from_later": true,
            "profile": {"env": [["GOOD", "1"], ["BAD=KEY", "2"], ["", "3"]]}
        }],
        "current": 0,
        "recall": {"0": {"tabs": [
            {"id": 1, "title": "ok", "tree": {"nodes": [{"kind": "leaf", "v": 1}]}},
            {"id": 2, "title": "damaged", "tree": {"nodes": [{"kind": "leaf", "v": 1}, {"kind": "leaf", "v": 2}]}}
        ], "focused": 1}}
    });
    let spaces: TemorSpaces = serde_json::from_value(file).unwrap();
    let project = &spaces.current().payload;
    assert_eq!(project.root, None);
    let keys: Vec<&str> = project
        .profile
        .env
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    assert_eq!(keys, vec!["GOOD"], "bad keys are dropped, not the profile");
    let place = spaces.recall().of(SpaceId(0));
    let titles: Vec<&str> = place.tabs.iter().map(|tab| tab.title.as_str()).collect();
    assert_eq!(titles, vec!["ok"], "a damaged tab costs only itself");
    assert_eq!(place.tabs[0].pin, Pin::Loose);
}

#[test]
fn one_bad_field_costs_only_itself() {
    let file = json!({
        "spaces": [
            {"id": 0, "name": "Work", "root": 5, "profile": {"ssh_identity": [], "dotenv": ".env"}},
            {"id": 1, "name": "Home", "profile": 7, "routes": [{"pattern": "ssh h-*"}]}
        ],
        "current": 0,
        "recall": {
            "0": {"tabs": [{"id": 1, "title": "ok", "pin": 3, "folder": "x",
                            "tree": {"nodes": [{"kind": "leaf", "v": 1}]}}], "focused": "x"},
            "1": 5
        }
    });
    let spaces: TemorSpaces = serde_json::from_value(file).unwrap();
    let work = &spaces.list()[0].payload;
    assert_eq!(work.root, None);
    assert_eq!(work.profile.ssh_identity, None);
    assert_eq!(work.profile.dotenv, Some(RelPath(".env".into())));
    let home = &spaces.list()[1].payload;
    assert_eq!(home.profile, Profile::default());
    assert_eq!(home.routes.len(), 1);
    let place = spaces.recall().of(SpaceId(0));
    assert_eq!(place.focused, None);
    assert_eq!(place.tabs.len(), 1, "a bad focused keeps the tabs");
    assert_eq!(
        (place.tabs[0].pin.clone(), place.tabs[0].folder),
        (Pin::Loose, None)
    );
    assert_eq!(spaces.recall().of(SpaceId(1)), Place::default());
}
