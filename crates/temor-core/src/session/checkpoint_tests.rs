//! The checkpoint file: what is written reads back, and a file that is odd is read as far as it
//! can be.

use proptest::prelude::*;

use super::{Checkpoint, CheckpointFault, CommandRecord, MOST_HISTORY, VERSION};
use crate::block::{Author, Block};
use crate::session::Replay;
use crate::vocab::ids::{BlockId, ExitCode, GridRow, GridSpan};
use crate::vocab::text::CommandText;
use ds_core::time::stamp::Stamp;

fn block(id: u64, command: &str, ended: Option<u64>, exit: Option<i32>) -> Block {
    Block {
        id: BlockId(id),
        prompt: GridSpan::row(GridRow(0)),
        command: CommandText(command.to_owned()),
        output: GridSpan::row(GridRow(1)),
        exit: exit.map(ExitCode),
        by: Author::Person,
        started: Stamp(1000),
        ended: ended.map(Stamp),
    }
}

fn replay() -> Replay {
    Replay {
        cwd: Some("/work".into()),
        env: vec![("EDITOR".into(), "vim".into())],
        aliases: vec![("g".into(), "git".into())],
        withheld: vec!["GITHUB_TOKEN".into()],
    }
}

#[test]
fn a_checkpoint_keeps_the_state_and_the_commands_that_finished() {
    let blocks = [
        block(1, "ls", Some(1040), Some(0)),
        block(2, "sleep 9", None, None),
        block(3, "false", Some(1010), Some(1)),
    ];
    let checkpoint = Checkpoint::of(replay(), blocks.iter());
    assert_eq!(checkpoint.version, VERSION);
    assert_eq!(
        checkpoint.history,
        [
            CommandRecord {
                command: "ls".into(),
                exit: Some(0),
                took_ms: Some(40)
            },
            CommandRecord {
                command: "false".into(),
                exit: Some(1),
                took_ms: Some(10)
            },
        ],
        "a running command is not history"
    );
    assert_eq!(checkpoint.replay(), replay());
    assert_eq!(Checkpoint::decode(&checkpoint.encode()), Ok(checkpoint));
}

#[test]
fn only_the_newest_commands_are_kept() {
    let blocks: Vec<Block> = (0..MOST_HISTORY as u64 + 7)
        .map(|n| block(n, &format!("c{n}"), Some(1001), Some(0)))
        .collect();
    let checkpoint = Checkpoint::of(Replay::default(), blocks.iter());
    assert_eq!(checkpoint.history.len(), MOST_HISTORY);
    assert_eq!(checkpoint.history[0].command, "c7");
}

#[test]
fn a_file_is_read_as_far_as_it_can_be() {
    // (what, text, the folder it yields, or the fault)
    let cases = [
        ("not JSON", "nope", Err(CheckpointFault::Damaged)),
        ("not an object", "[1]", Err(CheckpointFault::Damaged)),
        (
            "no version",
            r#"{"cwd":"/a"}"#,
            Err(CheckpointFault::Damaged),
        ),
        (
            "a newer build's",
            r#"{"version":9,"cwd":"/a"}"#,
            Err(CheckpointFault::Newer(9)),
        ),
        (
            "extra fields",
            r#"{"version":1,"cwd":"/a","future":{"x":1}}"#,
            Ok(Some("/a")),
        ),
        (
            "a damaged field beside good ones",
            r#"{"version":1,"cwd":"/a","env":"oops","history":[{"command":"ls"}]}"#,
            Ok(Some("/a")),
        ),
    ];
    for (what, text, want) in cases {
        let got = Checkpoint::decode(text).map(|c| c.cwd);
        assert_eq!(got, want.map(|cwd| cwd.map(str::to_owned)), "{what}");
    }
    let odd = Checkpoint::decode(r#"{"version":1,"env":"oops","history":[{"command":"ls"}]}"#)
        .expect("reads");
    assert_eq!(odd.env, []);
    assert_eq!(odd.history.len(), 1, "the good field is kept");
}

proptest! {
    #[test]
    fn whatever_is_kept_reads_back(
        cwd in proptest::option::of(".{0,20}"),
        env in proptest::collection::vec((".{1,8}", ".{0,12}"), 0..6),
        history in proptest::collection::vec(
            (".{0,20}", proptest::option::of(-3i32..300), proptest::option::of(0u64..100000)), 0..6),
    ) {
        let checkpoint = Checkpoint {
            version: VERSION,
            cwd,
            env,
            aliases: vec![],
            withheld: vec![],
            history: history
                .into_iter()
                .map(|(command, exit, took_ms)| CommandRecord { command, exit, took_ms })
                .collect(),
            daemon_session: Some(7),
        };
        prop_assert_eq!(Checkpoint::decode(&checkpoint.encode()), Ok(checkpoint));
    }
}

#[test]
fn commands_of_a_shell_that_goes_on_come_first_and_only_the_newest_are_kept() {
    let record = |n: usize| CommandRecord {
        command: format!("c{n}"),
        exit: Some(0),
        took_ms: None,
    };
    let earlier: Vec<_> = (0..MOST_HISTORY).map(record).collect();
    let now = Checkpoint {
        history: vec![record(1000), record(1001)],
        ..Checkpoint::default()
    };
    let joined = now.after(&earlier);
    assert_eq!(joined.history.len(), MOST_HISTORY);
    assert_eq!(
        joined.history.last().map(|r| r.command.as_str()),
        Some("c1001")
    );
    assert_eq!(
        joined.history.first().map(|r| r.command.as_str()),
        Some("c2")
    );
}
