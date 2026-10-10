use super::{
    Line, OfferIn, OfferList, OfferOut, OfferState, Program, Reply, Screen, asks_for_secret,
    offer_step,
};
use crate::vocab::ids::BlockId;
use ds_core::time::stamp::Stamp;

fn program(text: &str) -> Option<Program> {
    Some(Program(text.to_owned()))
}

#[test]
fn matching_reads_the_first_words_of_the_command() {
    let list = OfferList::default();
    let rows: [(&str, Option<&str>); 22] = [
        ("ssh host", Some("ssh")),
        ("/usr/bin/ssh -p 22 host", Some("ssh")),
        ("TERM=xterm ssh host", Some("ssh")),
        ("mosh host", Some("mosh")),
        ("docker exec -it web sh", Some("docker exec")),
        ("docker run --rm -it ubuntu bash", Some("docker run -it")),
        ("docker run -ti ubuntu", Some("docker run -it")),
        ("docker run ubuntu", None),
        ("podman exec -it x sh", Some("podman exec")),
        ("kubectl exec -it pod -- sh", Some("kubectl exec")),
        ("sudo -i", Some("sudo -i")),
        ("sudo -u bob -s", Some("sudo -s")),
        ("sudo apt update", None),
        ("su", Some("su")),
        ("su - root", Some("su")),
        ("sudo", None),
        ("nix-shell -p git", Some("nix-shell")),
        ("nix develop", Some("nix develop")),
        ("nix build", None),
        ("poetry shell", Some("poetry shell")),
        ("echo ssh host", None),
        ("", None),
    ];
    for (command, want) in rows {
        assert_eq!(list.matches(command), want.and_then(program), "{command:?}");
    }
}

#[test]
fn the_lists_add_and_remove() {
    let add = [
        "kubectl debug -it".to_owned(),
        "ssh".to_owned(),
        "-x".to_owned(),
    ];
    let remove = ["su".to_owned(), "docker  exec".to_owned()];
    let list = OfferList::new(&add, &remove);
    assert_eq!(
        list.matches("kubectl debug -it pod"),
        program("kubectl debug -it")
    );
    assert_eq!(list.matches("kubectl debug pod"), None);
    assert_eq!(list.matches("su"), None);
    assert_eq!(list.matches("docker exec c sh"), None);
    assert_eq!(list.matches("ssh h"), program("ssh"));
    let mut list = list;
    list.remove(&Program("ssh".to_owned()));
    assert_eq!(list.matches("ssh h"), None);
}

#[test]
fn secret_prompts_are_recognised() {
    for line in [
        "user@host's password: ",
        "Enter passphrase for key '/home/a/.ssh/id':",
        "Are you sure you want to continue connecting (yes/no/[fingerprint])?",
        "Verification code:",
    ] {
        assert!(asks_for_secret(line), "{line}");
    }
    assert!(!asks_for_secret("root@web:~# "));
}

fn at(ms: u64) -> Stamp {
    Stamp(ms)
}
fn running(block: u64, name: &str, ms: u64) -> OfferIn {
    OfferIn::Running {
        now: Some((BlockId(block), Program(name.to_owned()))),
        at: at(ms),
    }
}
fn tick(ms: u64) -> OfferIn {
    OfferIn::Tick {
        at: at(ms),
        screen: Screen::Normal,
        line: Line::Plain,
    }
}

/// Feeds `inputs` in order; returns the final state and every output.
fn run(inputs: Vec<OfferIn>) -> (OfferState, Vec<OfferOut>) {
    let mut state = OfferState::Idle;
    let mut outs = Vec::new();
    for input in inputs {
        let (next, mut out) = offer_step(state, input);
        state = next;
        outs.append(&mut out);
    }
    (state, outs)
}

fn asking(state: &OfferState) -> bool {
    state.asking().is_some()
}

#[test]
fn the_offer_waits_for_the_timing_rule() {
    // (name, inputs, offer showing at the end)
    let alt = OfferIn::Tick {
        at: at(3_000),
        screen: Screen::Alternate,
        line: Line::Plain,
    };
    let secret = OfferIn::Tick {
        at: at(3_000),
        screen: Screen::Normal,
        line: Line::Asking,
    };
    let rows: Vec<(&str, Vec<OfferIn>, bool)> = vec![
        ("too soon", vec![running(1, "ssh", 0), tick(1_400)], false),
        (
            "quiet and long enough",
            vec![running(1, "ssh", 0), tick(1_500)],
            true,
        ),
        (
            "output still arriving",
            vec![
                running(1, "ssh", 0),
                OfferIn::Output { at: at(1_300) },
                tick(1_500),
            ],
            false,
        ),
        (
            "output paused for half a second",
            vec![
                running(1, "ssh", 0),
                OfferIn::Output { at: at(1_300) },
                tick(1_800),
            ],
            true,
        ),
        ("alternate screen", vec![running(1, "ssh", 0), alt], false),
        (
            "asks for a password",
            vec![running(1, "ssh", 0), secret],
            false,
        ),
        (
            "command ended first",
            vec![
                running(1, "ssh", 0),
                OfferIn::Running {
                    now: None,
                    at: at(1_000),
                },
                tick(3_000),
            ],
            false,
        ),
        (
            "output under the offer takes it back",
            vec![
                running(1, "ssh", 0),
                tick(2_000),
                OfferIn::Output { at: at(2_100) },
            ],
            false,
        ),
        (
            "and it comes again after quiet",
            vec![
                running(1, "ssh", 0),
                tick(2_000),
                OfferIn::Output { at: at(2_100) },
                tick(4_700),
            ],
            true,
        ),
    ];
    for (name, inputs, want) in rows {
        let (state, _) = run(inputs);
        assert_eq!(asking(&state), want, "{name}");
    }
}

#[test]
fn replies_settle_the_command_and_say_what_follows() {
    let base = vec![running(4, "docker exec", 0), tick(2_000)];
    let rows = [
        (Reply::Yes, vec![OfferOut::Redraw, OfferOut::Load]),
        (Reply::NotNow, vec![OfferOut::Redraw]),
        (
            Reply::Never,
            vec![
                OfferOut::Redraw,
                OfferOut::Never(Program("docker exec".to_owned())),
            ],
        ),
    ];
    for (reply, want) in rows {
        let mut inputs = base.clone();
        inputs.push(OfferIn::Reply(reply));
        // Time passing and the same command still running ask nothing more.
        inputs.push(tick(9_000));
        inputs.push(running(4, "docker exec", 9_000));
        let (state, outs) = run(inputs);
        assert_eq!(
            state,
            OfferState::Settled { block: BlockId(4) },
            "{reply:?}"
        );
        let tail: Vec<OfferOut> = outs.into_iter().skip(1).collect();
        assert_eq!(tail, want, "{reply:?}");
    }
}

#[test]
fn a_reply_with_no_offer_does_nothing_and_a_new_command_starts_over() {
    let (state, outs) = run(vec![running(1, "ssh", 0), OfferIn::Reply(Reply::Yes)]);
    assert!(outs.is_empty());
    assert!(matches!(state, OfferState::Watching { .. }));
    let (state, _) = run(vec![
        running(1, "ssh", 0),
        tick(2_000),
        OfferIn::Reply(Reply::NotNow),
        running(2, "su", 10_000),
        tick(12_000),
    ]);
    assert_eq!(state.asking(), program("su").as_ref());
}
