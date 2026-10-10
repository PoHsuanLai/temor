use super::*;
use temor_desktop::{Exit, Focus, PaneKind, Placement};

fn pane(id: u64) -> PaneInfo {
    PaneInfo {
        id: PaneRef(id),
        title: "zsh".to_owned(),
        cwd: None,
        kind: PaneKind::Terminal,
        focus: Focus::Focused,
    }
}

#[test]
fn reads_return_the_last_snapshot_and_an_unknown_pane_is_refused() {
    let (port, link, _count) = desktop_pair();
    let mut snapshot = Snapshot::empty();
    snapshot.panes = vec![pane(1)];
    let ls = CommandInfo {
        id: CommandRef(3),
        line: "ls".to_owned(),
        exit: Exit::Code(0),
    };
    snapshot.commands.insert(PaneRef(1), vec![ls.clone()]);
    link.publish(snapshot, BadgeCount(0));

    assert_eq!(port.panes(), vec![pane(1)]);
    assert_eq!(port.commands(PaneRef(1)), Ok(vec![ls]));
    assert_eq!(port.commands(PaneRef(9)), Err(PortRefusal::NoSuchPane));
}

#[test]
fn an_action_is_answered_by_the_window_thread_and_refused_once_it_is_gone() {
    let (port, link, _count) = desktop_pair();
    let mut asks = link.take_asks().expect("the asks");
    let window = std::thread::spawn(move || {
        let Some(Ask::Act(act, reply)) = asks.blocking_recv() else {
            return;
        };
        let answer = match act {
            Act::Open(_) => Ok(Answer::Pane(PaneRef(7))),
            _ => Err(PortRefusal::NoSuchPane),
        };
        let _ = reply.send(answer);
    });
    let open = PaneOpen {
        kind: PaneKind::Terminal,
        placement: Placement::Tab,
        cwd: None,
    };
    assert_eq!(port.open_pane(open), Ok(PaneRef(7)));
    window.join().expect("the window thread");
    assert_eq!(port.close_pane(PaneRef(7)), Err(PortRefusal::Busy));
}

#[test]
fn the_dock_hears_only_a_changed_count() {
    let (_port, link, mut count) = desktop_pair();
    let steps = [(0, false), (2, true), (2, false), (0, true)];
    for (shown, heard) in steps {
        link.publish(Snapshot::empty(), BadgeCount(shown));
        assert_eq!(count.has_changed().expect("the sender"), heard, "{shown}");
        let _ = count.borrow_and_update();
    }
}
