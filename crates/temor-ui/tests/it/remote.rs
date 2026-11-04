//! A pane on a host's helper, against a helper in this process over pipes: its output shows, and
//! when the link drops and comes back, the pane gets its screen again and goes on (PLAN §9b).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use temor_client::remote::conn::Conn;
use temor_core::vocab::ids::PaneId;
use temor_pane::{FedSpec, PaneEvent, PaneHandle, start_fed};
use temor_panes::remote::RemoteWork;
use temor_panes::remote::event::{Generation, RemoteEvent, Ticket, Wanted};
use temor_panes::remote::link::{RemoteLink, Target, lock};
use temor_panes::remote::palette::wire_palette;
use temor_panes::remote::pump::pump;
use temor_panes::remote::route::Router;
use temor_remote::{Client, DAEMON_CAPS, Daemon, hello};
use temor_render::Palette;
use temor_ssh::HostAlias;
use temor_vt::{CellPx, CursorShape, GridSize, LineCount};
use temor_wire::digest::ContentHash;
use temor_wire::hello::HelperVersion;
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use super::support::{bash_path, helper_daemon, scratch, tools_present};

const HELPER: HelperVersion = HelperVersion(ContentHash([7; 32]));
const PATIENCE: Duration = Duration::from_secs(20);

/// One link to the daemon, as a window would hold it: the connection, its router and the
/// thread that reads its events into the router.
struct Link {
    conn: Arc<Conn>,
    router: Router,
    notes: UnboundedReceiver<RemoteEvent>,
    work: RemoteWork,
}

fn connect(daemon: &Arc<Daemon>, generation: Generation) -> Link {
    let (served_in, client_out) = std::io::pipe().expect("pipe");
    let (client_in, served_out) = std::io::pipe().expect("pipe");
    let serving = Arc::clone(daemon);
    std::thread::spawn(move || serving.serve(served_in, served_out));
    let (client, incoming) =
        Client::connect(client_in, client_out, hello(HELPER, &DAEMON_CAPS)).expect("greeting");
    let router = Router::default();
    let (tell, notes) = unbounded_channel();
    let (host, reading) = (HostAlias::new("box").expect("a name"), router.clone());
    let tell_pump = tell.clone();
    std::thread::spawn(move || pump(&host, generation, &incoming, &reading, &tell_pump));
    Link {
        conn: Arc::new(Conn::bare(client)),
        router,
        notes,
        work: RemoteWork::new(tell),
    }
}

fn next_note(
    notes: &mut UnboundedReceiver<RemoteEvent>,
    want: impl Fn(&RemoteEvent) -> bool,
) -> RemoteEvent {
    let until = Instant::now() + PATIENCE;
    loop {
        match notes.try_recv() {
            Ok(note) if want(&note) => return note,
            Ok(_) => {}
            Err(_) if Instant::now() > until => panic!("the event never came"),
            Err(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

fn rows(pane: &PaneHandle) -> Vec<String> {
    pane.with_term(|term| {
        (0..term.screen_rows().unwrap_or(0))
            .map(|row| {
                term.row_text(row)
                    .map(|line| line.text().trim_end().to_owned())
                    .unwrap_or_default()
            })
            .collect()
    })
}

fn wait_for_row(pane: &PaneHandle, text: &str) {
    let until = Instant::now() + PATIENCE;
    while !rows(pane).iter().any(|row| row == text) {
        assert!(
            Instant::now() < until,
            "no row {text:?}; the screen shows {:?}",
            rows(pane)
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn saw_reset(events: &Receiver<PaneEvent>) -> bool {
    let until = Instant::now() + PATIENCE;
    while Instant::now() < until {
        if let Ok(PaneEvent::Reset) = events.recv_timeout(Duration::from_millis(50)) {
            return true;
        }
    }
    false
}

#[test]
fn a_pane_on_the_helper_shows_its_output_and_gets_its_screen_back_after_the_link_drops() {
    if !tools_present() {
        return;
    }
    let dir = scratch("remote-pane");
    let home = dir.join("home");
    let daemon = helper_daemon(HELPER, &dir, &home, bash_path());

    // The pane: fed from the helper, its input going back through a link of its own.
    let target = Arc::new(std::sync::Mutex::new(Target::default()));
    let link = RemoteLink::start(Arc::clone(&target)).expect("the writer thread starts");
    let (sink, events) = channel();
    let (pane, runner, feeder) = start_fed(
        FedSpec {
            link,
            grid: GridSize::new(80, 24),
            cell: CellPx::new(8, 16),
            scrollback: LineCount(1000),
            palette: Palette::default(),
            cursor: CursorShape::Block,
            transfers: None,
        },
        Arc::new(sink),
    )
    .expect("a fed pane");
    std::thread::spawn(move || runner.run());

    // First link: open a session, type into it.
    let host = HostAlias::new("box").expect("a name");
    let mut first = connect(&daemon, Generation(1));
    let env = vec![
        ("HOME".to_owned(), home.display().to_string()),
        ("PATH".to_owned(), "/usr/bin:/bin".to_owned()),
        ("TERM".to_owned(), "dumb".to_owned()),
        ("PS1".to_owned(), "$ ".to_owned()),
        ("TEMOR_PROMPT".to_owned(), "shell".to_owned()),
    ];
    first.work.attach(
        Arc::clone(&first.conn),
        first.router.clone(),
        (host.clone(), Generation(1)),
        Ticket {
            pane: PaneId(1),
            target: Arc::clone(&target),
            feeder: feeder.clone(),
            palette: wire_palette(&Palette::default()),
            wanted: Wanted::Fresh {
                cwd: home.display().to_string(),
                env,
                shell: None,
            },
        },
    );
    let RemoteEvent::Attached { session, .. } = next_note(&mut first.notes, |note| {
        matches!(note, RemoteEvent::Attached { .. })
    }) else {
        unreachable!("the filter keeps only Attached")
    };
    pane.send(b"echo marker-one\r");
    wait_for_row(&pane, "marker-one");

    // The link drops: the pane stops sending, and the helper keeps the session.
    lock(&target).conn = None;
    drop(first.conn);
    next_note(&mut first.notes, |note| {
        matches!(note, RemoteEvent::Closed { .. })
    });

    // The next link reattaches it, and the pane has its screen again.
    let mut second = connect(&daemon, Generation(2));
    second.work.attach(
        Arc::clone(&second.conn),
        second.router.clone(),
        (host, Generation(2)),
        Ticket {
            pane: PaneId(1),
            target: Arc::clone(&target),
            feeder,
            palette: wire_palette(&Palette::default()),
            wanted: Wanted::Again(session),
        },
    );
    next_note(&mut second.notes, |note| {
        matches!(note, RemoteEvent::Attached { .. })
    });
    assert!(
        saw_reset(&events),
        "the pane started over from the host's screen"
    );
    wait_for_row(&pane, "marker-one");

    // And the live output goes on.
    pane.send(b"echo marker-two\r");
    wait_for_row(&pane, "marker-two");
}
