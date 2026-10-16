//! The pipeline's last command, read from a fake `/proc` in a scratch folder.

use std::path::Path;

use crate::pty::{Foreground, ProcRoot, ProcessId};

/// One process in the fake `/proc`: its name, process group and whether it is a child listed
/// under the shell (pid 10).
struct Proc {
    pid: i32,
    comm: &'static str,
    group: i32,
    /// Clock ticks since boot when it started (`stat` field 22).
    start: u64,
}

fn write_proc(root: &Path, procs: &[Proc], children_of_shell: &str) {
    for p in procs {
        let dir = root.join(p.pid.to_string());
        std::fs::create_dir_all(dir.join("task").join(p.pid.to_string())).expect("proc dir");
        std::fs::write(dir.join("comm"), format!("{}\n", p.comm)).expect("comm");
        // The name may hold spaces and parentheses; fields count from its last `)`.
        std::fs::write(
            dir.join("stat"),
            format!(
                "{} ({} (x)) S 10 {} 10 0 -1 {}{}\n",
                p.pid,
                p.comm,
                p.group,
                "0 ".repeat(13),
                p.start
            ),
        )
        .expect("stat");
    }
    std::fs::write(
        root.join("10/task/10/children"),
        format!("{children_of_shell}\n"),
    )
    .expect("children");
}

/// A case: name, shell, the processes, the group, and the name wanted.
type Case<'a> = (&'a str, &'a str, Vec<Proc>, i32, Option<&'a str>);

#[test]
fn a_pipeline_is_named_for_its_last_command() {
    let shell = Proc {
        pid: 10,
        comm: "bash",
        group: 10,
        start: 1,
    };
    let job = |pid, comm, group| Proc {
        pid,
        comm,
        group,
        start: u64::try_from(pid).unwrap_or(0),
    };
    // (name, children of the shell, processes besides the shell, the foreground group, the name)
    let cases: Vec<Case> = vec![
        (
            "three commands",
            "11 12 13",
            vec![job(11, "cat", 11), job(12, "grep", 11), job(13, "less", 11)],
            11,
            Some("less"),
        ),
        (
            "pids wrapped: the one started last wins, not the higher pid",
            "30 12",
            vec![
                Proc {
                    pid: 30,
                    comm: "cat",
                    group: 30,
                    start: 500,
                },
                Proc {
                    pid: 12,
                    comm: "wc",
                    group: 30,
                    start: 501,
                },
            ],
            30,
            Some("wc"),
        ),
        (
            "a command alone",
            "11",
            vec![job(11, "sleep", 11)],
            11,
            Some("sleep"),
        ),
        (
            "a background job in another group is not part of it",
            "11 12 14",
            vec![job(11, "cat", 11), job(12, "wc", 11), job(14, "make", 14)],
            11,
            Some("wc"),
        ),
        (
            "no list of children: the leader's name",
            "",
            vec![job(11, "cat", 11), job(12, "wc", 11)],
            11,
            Some("cat"),
        ),
        (
            "the shell itself in front",
            "11",
            vec![job(11, "cat", 11)],
            10,
            Some("bash"),
        ),
    ];
    for (name, children, others, group, want) in cases {
        let scratch = tempfile::tempdir().expect("tempdir");
        let mut procs = vec![Proc {
            pid: shell.pid,
            comm: shell.comm,
            group: shell.group,
            start: shell.start,
        }];
        procs.extend(others);
        write_proc(scratch.path(), &procs, children);
        let proc = ProcRoot(scratch.path().to_path_buf());
        let got = Foreground::of_pipeline(ProcessId(group), ProcessId(10), &proc);
        assert_eq!(got.name.as_deref(), want, "{name}");
        assert_eq!(got.group, ProcessId(group), "{name}");
    }
}
