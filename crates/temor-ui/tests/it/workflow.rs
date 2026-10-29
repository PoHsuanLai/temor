//! One person's working session, end to end in ds-harness (event dispatch only, never OS input):
//! a local project, `rsync` to a real scratch sshd on 127.0.0.1 (`realhost`), a host tab opened
//! from the bar, a fake agent in a split, uploads by drag, downloads through the terminal, a
//! resize while a question shows, closing a tab mid-transfer, and a drop on a local pane.
//!
//! The sshd runs as this user with a scratch host key, authorized key and known_hosts; its forced
//! command swaps in a scratch HOME. An `ssh` wrapper first on the panes' `PATH` (and the `PATH`
//! temor finds `ssh` on) adds `-F` with a scratch client config, so neither temor nor `rsync -e
//! ssh` reads the real `~/.ssh`. The "agent" is a shell script. Each step prints one
//! `STEP_PASS|id|evidence` or `STEP_FAIL|id|why` line; the test fails if any step failed.
//!
//! Not part of the regular run or CI: it takes minutes and needs sshd, rsync and the host helper
//! built. `scripts/e2e-workflow.sh` builds the helper and runs it.

use std::fs;
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use ds::base::vocab::ShortcutKey;
use ds::file_drop::drag::{FileDragInput, Offer};
use ds::prelude::{Point, Px};
use ds_blitz::Extent;
use ds_harness::{Harness, Input};
use temor_testkit::git;
use temor_ui::view::probe::Probe;

use super::drive;
use super::drive::{chord, rows, type_line, type_text};
use super::support::{ScratchRoot, heavy, socket_root, tools_present};
use super::window::harness_with;

const NAME: &str = "workflow";
const WAIT: Duration = Duration::from_secs(60);

// ---------------------------------------------------------------- the scratch sshd

/// A running sshd that `realhost` reaches, and the files around it. Killed on drop.
struct Sshd {
    root: PathBuf,
    /// The folder with the `ssh` wrapper and the fake agent.
    bin: PathBuf,
    /// The client config the wrapper passes with `-F`.
    config: PathBuf,
    /// The scratch HOME the forced command gives every session on `realhost`.
    remote_home: PathBuf,
    /// temor's runtime folder: short, as the shared connection's socket path must fit 108 bytes.
    sockets: ScratchRoot,
    child: Child,
}

impl Drop for Sshd {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // The helper daemon and its shells outlive sshd (they are detached by design), as does
        // the shared connection's ssh master; they run from the scratch HOME under `root` or on
        // a socket under `sockets`, so only this fixture's processes match.
        for dir in [self.root.as_path(), &self.sockets] {
            let _ = Command::new("pkill")
                .arg("-f")
                .arg(format!("{}/", dir.display()))
                .status();
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn which(name: &str) -> Option<PathBuf> {
    temor_pty::find_program(name, std::ffi::OsStr::new("/usr/sbin:/usr/bin:/sbin:/bin"))
}

fn executable(path: &Path, text: &str) {
    fs::write(path, text).expect("write");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

fn keygen(tool: &Path, file: &Path) {
    let ok = Command::new(tool)
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(file)
        .stdin(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "ssh-keygen {}", file.display());
}

/// Starts the sshd, or says why not (a skip).
fn sshd() -> Result<Sshd, String> {
    let mut tools = Vec::new();
    for name in ["sshd", "ssh", "ssh-keygen", "rsync", "git", "bash"] {
        tools.push(which(name).ok_or_else(|| format!("{name} is not installed"))?);
    }
    let (sshd, ssh, ssh_keygen, bash) = (&tools[0], &tools[1], &tools[2], &tools[5]);
    // The scratch host and its HOME; temor's own sockets go under `socket_root`.
    let root = std::env::temp_dir().join(format!("twf-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let (work, bin) = (root.join("w"), root.join("bin"));
    let (remote_home, remote_tmp) = (root.join("rhome"), root.join("rtmp"));
    for dir in [&work, &bin, &remote_home, &remote_tmp] {
        fs::create_dir_all(dir).expect("scratch folder");
    }
    keygen(ssh_keygen, &work.join("host_key"));
    keygen(ssh_keygen, &work.join("client_key"));
    fs::copy(work.join("client_key.pub"), work.join("authorized_keys")).expect("authorized");
    let port = TcpListener::bind(("127.0.0.1", 0))
        .and_then(|l| l.local_addr())
        .expect("a free port")
        .port();
    let user = std::env::var("USER").map_err(|_| "USER is not set".to_owned())?;
    let (wd, rh, rt) = (work.display(), remote_home.display(), remote_tmp.display());
    let bash = bash.display();
    // No command: an interactive shell (a plain `ssh realhost`); a command: run it.
    executable(
        &work.join("force.sh"),
        &format!(
            "#!/bin/sh\nexport HOME={rh} TMPDIR={rt} HISTFILE=/dev/null SHELL={bash} \
             PATH=/usr/bin:/bin\ncd \"$HOME\" || exit 70\n\
             [ -n \"$SSH_ORIGINAL_COMMAND\" ] || exec {bash} -i\n\
             exec /bin/sh -c \"$SSH_ORIGINAL_COMMAND\"\n"
        ),
    );
    fs::write(
        work.join("sshd_config"),
        format!(
            "ListenAddress 127.0.0.1\nPort {port}\nHostKey {wd}/host_key\n\
             AuthorizedKeysFile {wd}/authorized_keys\nPidFile none\nUsePAM no\nStrictModes no\n\
             PasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitUserRC no\n\
             PermitUserEnvironment no\nAllowTcpForwarding local\nForceCommand {wd}/force.sh\n"
        ),
    )
    .expect("sshd_config");
    let config = work.join("ssh_config");
    fs::write(
        &config,
        format!(
            "Host realhost\n  HostName 127.0.0.1\n  Port {port}\n  User {user}\n\
             IdentityFile {wd}/client_key\n  IdentitiesOnly yes\n\
             UserKnownHostsFile {wd}/known_hosts\n  GlobalKnownHostsFile /dev/null\n\
             StrictHostKeyChecking accept-new\n  BatchMode yes\n  LogLevel ERROR\n"
        ),
    )
    .expect("ssh_config");
    executable(
        &bin.join("ssh"),
        &format!(
            "#!/bin/sh\necho \"$*\" >> {}/ssh-args.log\nexec {} -F {} \"$@\"\n",
            work.display(),
            ssh.display(),
            config.display()
        ),
    );
    // The fake agent: names itself as one, works, then waits for an answer.
    executable(
        &bin.join("claude"),
        "#!/bin/sh\nprintf '\\033]7501;state=working:app=claude\\007'\n\
         echo FAKE-AGENT-WORKING\nsleep 2\n\
         printf '\\033]7501;state=blocked:kind=permission:app=claude\\007'\n\
         echo FAKE-AGENT-WAITING\nread answer\n",
    );
    let log = fs::File::create(work.join("sshd.log")).expect("log");
    let child = Command::new(sshd)
        .args(["-D", "-e", "-f"])
        .arg(work.join("sshd_config"))
        .stdin(Stdio::null())
        .stdout(log.try_clone().expect("log"))
        .stderr(log)
        .spawn()
        .map_err(|e| format!("sshd did not start: {e}"))?;
    let sockets = socket_root("workflow", "");
    let fixture = Sshd {
        sockets,
        root,
        bin,
        config,
        remote_home,
        child,
    };
    let up = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        if up.elapsed() > Duration::from_secs(10) {
            let log = fs::read_to_string(work.join("sshd.log")).unwrap_or_default();
            return Err(format!("sshd never listened: {log}"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    // The wrapper reaches it, and rsync is on the far side.
    let probe = Command::new(fixture.bin.join("ssh"))
        .args(["realhost", "command -v rsync && echo SSH-OK"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("ssh wrapper: {e}"))?;
    if !String::from_utf8_lossy(&probe.stdout).contains("SSH-OK") {
        return Err(format!(
            "the scratch sshd is not usable: {}",
            String::from_utf8_lossy(&probe.stderr)
        ));
    }
    Ok(fixture)
}

// ---------------------------------------------------------------- the steps

/// The steps' markers, printed as they finish.
#[derive(Default)]
struct Steps {
    lines: Vec<String>,
    failed: usize,
}

fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(2500).collect()
}

impl Steps {
    fn run(&mut self, id: &str, step: impl FnOnce() -> String) {
        // `WF_STEPS=W3,W9` runs only those steps (a quicker loop while one is looked into).
        if let Ok(only) = std::env::var("WF_STEPS")
            && !only.split(',').any(|s| s.trim() == id)
        {
            eprintln!("STEP_SKIP|{id}");
            return;
        }
        let line = match catch_unwind(AssertUnwindSafe(step)) {
            Ok(evidence) => format!("STEP_PASS|{id}|{}", one_line(&evidence)),
            Err(payload) => {
                self.failed += 1;
                let why = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
                    .unwrap_or_else(|| "panicked".to_owned());
                format!("STEP_FAIL|{id}|{}", one_line(&why))
            }
        };
        eprintln!("{line}");
        self.lines.push(line);
    }
}

// ---------------------------------------------------------------- driving the window

/// Waits for `ok` on the terminal's text, up to [`WAIT`].
fn wait_screen(h: &mut Harness, probe: &Probe, what: &str, ok: impl Fn(&str) -> bool) {
    drive::wait_screen(h, probe, WAIT, what, ok);
}

thread_local! {
    /// The window's probe, so a timed-out wait can say what the screen showed.
    static SCREEN: std::cell::RefCell<Option<Probe>> = const { std::cell::RefCell::new(None) };
}

fn screen_tail() -> String {
    let text = SCREEN.with(|s| s.borrow().as_ref().map(Probe::screen).unwrap_or_default());
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(8)..].join(" / ")
}

fn wait_for(h: &mut Harness, what: &str, mut ok: impl FnMut(&mut Harness) -> bool) {
    let start = Instant::now();
    while !ok(h) {
        h.advance(Duration::from_millis(25));
        assert!(
            start.elapsed() < WAIT,
            "timed out waiting for {what}; cards {:?}; presence working={} waiting={}; screen: {}",
            metas(h),
            presence(h, "working"),
            presence(h, "waiting"),
            screen_tail()
        );
    }
}

fn click(h: &mut Harness, at: Point) {
    h.send(Input::click(at));
    h.advance(Duration::from_millis(150));
}

fn centre_of(rect: ds::prelude::Rect) -> Point {
    Point {
        x: Px(rect.origin.x.0 + rect.size.width.0 / 2.0),
        y: Px(rect.origin.y.0 + rect.size.height.0 / 2.0),
    }
}

/// The first terminal pane's rect (the left one of a split), and the whole pane area's right edge.
fn first_pane(h: &Harness) -> ds::prelude::Rect {
    h.rect("[aria-label=\"Terminal\"]")
        .unwrap_or_else(|| panic!("no terminal pane in the page"))
}

/// The finished command cards in the panes in front, each as its state and the status it says
/// ("done", "failed exit 1"). A card still running is left out, and so is the copy of a head
/// that sticks to the top of a list scrolled into its card (drawn after the list).
fn metas(h: &Harness) -> Vec<String> {
    let html = h.html();
    let list = html
        .split("class=\"temor-blk-sticky\"")
        .next()
        .unwrap_or_default();
    list.split("class=\"temor-bseg temor-blk-head\"")
        .skip(1)
        .filter_map(|part| {
            let state = part
                .split_once("data-state=\"")
                .and_then(|(_, rest)| rest.split_once('"'))
                .map(|(state, _)| state)?;
            if state == "running" {
                return None;
            }
            let status = part
                .split_once("class=\"temor-blk-meta\">")
                .and_then(|(_, rest)| rest.split_once('<'))
                .map(|(text, _)| text.split(" \u{b7} ").next().unwrap_or(text).trim())
                .filter(|_| state == "failed");
            Some(match status {
                Some(status) => format!("{state} {status}"),
                None => state.to_owned(),
            })
        })
        .collect()
}

fn presence(h: &Harness, which: &str) -> usize {
    h.html()
        .matches(&format!("data-presence=\"{which}\""))
        .count()
}

/// The label of each button in the first inline banner under `scope`.
fn banner_buttons(h: &Harness, scope: &str) -> Vec<(String, String)> {
    (1..=5)
        .filter_map(|n| {
            let sel = format!("{scope} .ds-inline-banner .ds-button:nth-of-type({n})");
            h.text_of(&sel).map(|text| (sel, text.trim().to_owned()))
        })
        .collect()
}

/// Presses a button in a banner under `scope` whose label is one of `yes`; the label pressed.
fn press_yes(h: &mut Harness, scope: &str, yes: &[&str]) -> Option<String> {
    let (sel, label) = banner_buttons(h, scope)
        .into_iter()
        .find(|(_, label)| yes.contains(&label.as_str()))?;
    let at = h.centre(&sel)?;
    click(h, at);
    Some(label)
}

fn drag_files(h: &mut Harness, at: Point, files: Vec<PathBuf>) {
    h.send(Input::FileDrag(FileDragInput::Entered { point: Some(at) }));
    h.send(Input::FileDrag(FileDragInput::Offered(Offer::Files(files))));
    h.send(Input::FileDrag(FileDragInput::Moved { point: at }));
    h.advance(Duration::from_millis(50));
    h.send(Input::FileDrag(FileDragInput::Dropped {
        modifiers: dioxus::html::Modifiers::empty(),
    }));
    h.advance(Duration::from_millis(300));
}

/// Opens the Downloads popover, returns its text, and closes it again.
fn shelf_text(h: &mut Harness) -> String {
    let trigger = h
        .centre(".temor-dl-hold .ds-button")
        .unwrap_or_else(|| panic!("no Downloads button in the head"));
    click(h, trigger);
    h.advance(Duration::from_millis(200));
    let text = h.text_of(".temor-dl-list").unwrap_or_default();
    let rows = h.count(".temor-dl");
    if let Some(again) = h.centre(".temor-dl-hold .ds-button") {
        click(h, again);
    }
    format!("[{rows} rows] {}", one_line(&text))
}

fn downloads(home: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(home.join("Downloads"))
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

/// `report.csv` holding "a,b\n1,2\n", printed through the terminal as iTerm2's File= sends it.
const PRINT_REPORT: &str =
    "printf '\\033]1337;File=name=cmVwb3J0LmNzdg==;size=8:YSxiCjEsMgo=\\033\\\\'";

// ---------------------------------------------------------------- the session

#[test]
#[ignore = "end to end: run with scripts/e2e-workflow.sh"]
fn a_working_session_across_a_local_project_a_real_host_an_agent_and_the_shelf() {
    if !tools_present() {
        return;
    }
    let _heavy = heavy();
    let host = match sshd() {
        Ok(host) => host,
        Err(why) => {
            eprintln!("SKIPPED: {why}");
            return;
        }
    };
    let helper = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .parent()
        .map(|target| target.join("debug").join("temor-remote"))
        .filter(|p| p.is_file());
    if helper.is_none() {
        eprintln!("GAP: no temor-remote helper build beside the tests; the host stays on Tier A");
    }
    let probe = Probe::new();
    let (bin, config, runtime) = (
        host.bin.clone(),
        host.config.clone(),
        host.sockets.to_path_buf(),
    );
    let mut h = harness_with(NAME, &probe, None, move |ambient| {
        let home = ambient.home.clone();
        // The project: a repository in the home folder, with proj/ to send.
        fs::create_dir_all(home.join("proj")).expect("proj");
        fs::write(home.join("proj/a.txt"), "alpha\n").expect("a");
        fs::write(home.join("proj/b.txt"), "beta\n").expect("b");
        git(&home, &["init", "-q", "-b", "main"]);
        git(&home, &["add", "proj"]);
        git(&home, &["commit", "-q", "-m", "first"]);
        // The host list temor offers reads the scratch home's ssh config.
        fs::create_dir_all(home.join(".ssh")).expect(".ssh");
        fs::copy(&config, home.join(".ssh/config")).expect("config");
        ambient
            .env
            .set("PATH", format!("{}:/usr/bin:/bin", bin.display()));
        ambient.env.set("TEMOR_PROMPT", "temor");
        if let Some(helper) = helper {
            ambient.env.set("TEMOR_REMOTE_HELPER", helper);
        }
        ambient.runtime_dir = runtime;
    });
    let home = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(NAME)
        .join("home");
    let remote_home = host.remote_home.clone();
    let mut steps = Steps::default();
    SCREEN.with(|s| *s.borrow_mut() = Some(probe.clone()));

    wait_screen(&mut h, &probe, "the first prompt", |s| s.contains("main"));
    let pane = first_pane(&h);
    click(&mut h, centre_of(pane));

    steps.run("W1", || {
        type_line(&mut h, "ls");
        wait_for(&mut h, "one card", |h| metas(h).len() == 1);
        type_line(&mut h, "git status");
        wait_for(&mut h, "two cards", |h| metas(h).len() == 2);
        let m = metas(&h);
        assert!(
            m.iter().all(|t| !t.contains("exit")),
            "a card failed: {m:?}"
        );
        let html = h.html();
        assert!(
            html.contains("class=\"temor-blk-where\""),
            "the composer names no folder"
        );
        format!("2 cards {m:?}, no exit; the composer names the folder")
    });

    steps.run("W2", || {
        // A clean screen, so the rsync card's header stays in view.
        type_line(&mut h, "clear");
        h.advance(Duration::from_millis(800));
        let before = metas(&h).len();
        type_line(&mut h, "rsync -av --info=progress2 proj/ realhost:proj/");
        wait_for(&mut h, "the rsync card", |h| metas(h).len() == before + 1);
        let m = metas(&h);
        let last = m.last().cloned().unwrap_or_default();
        assert!(
            !last.contains("exit"),
            "rsync failed: {last}; screen {}",
            probe.screen()
        );
        for f in ["a.txt", "b.txt"] {
            assert!(
                remote_home.join("proj").join(f).is_file(),
                "{f} is not on the host"
            );
        }
        format!("card {last:?} (exit 0); rhome/proj/a.txt,b.txt exist")
    });

    steps.run("W4", || {
        chord(&mut h, 'd');
        type_line(&mut h, "claude");
        wait_screen(&mut h, &probe, "the agent working", |s| {
            s.contains("FAKE-AGENT-WORKING")
        });
        let worked = presence(&h, "working");
        let start = Instant::now();
        while presence(&h, "waiting") == 0 && start.elapsed() < Duration::from_secs(10) {
            h.advance(Duration::from_millis(50));
        }
        if presence(&h, "waiting") == 0 {
            // Control: the same script under a name that is not an agent's.
            h.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('c')));
            h.advance(Duration::from_millis(300));
            type_line(&mut h, "cp \"$(command -v claude)\" ./fakeagent && ./fakeagent");
            let start = Instant::now();
            while presence(&h, "waiting") == 0 && start.elapsed() < Duration::from_secs(10) {
                h.advance(Duration::from_millis(50));
            }
            let renamed = (presence(&h, "waiting"), presence(&h, "working"));
            // Second control: the other pane of the same split, the shell line agents_window uses.
            h.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('c')));
            let left = first_pane(&h);
            click(&mut h, centre_of(left));
            type_line(
                &mut h,
                "printf '\\033]7501;state=blocked:kind=permission:app=claude\\007'; read answer",
            );
            let start = Instant::now();
            while presence(&h, "waiting") == 0 && start.elapsed() < Duration::from_secs(10) {
                h.advance(Duration::from_millis(50));
            }
            let first = presence(&h, "waiting");
            h.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('c')));
            h.advance(Duration::from_millis(300));
            panic!(
                "no orb for the `claude` script's OSC 7501 working/blocked in the split (working \
                 orbs {worked}; a split tab's row draws no orb, see agents_window's \
                 a_waiting_program_in_a_split_tab_still_shows_its_orb); same script named fakeagent: waiting={} working={}; the \
                 agents_window shell line in the left pane: waiting={first}",
                renamed.0, renamed.1
            );
        }
        // The keyboard to the other pane, then the chord brings it back to the agent.
        let left = first_pane(&h);
        click(&mut h, centre_of(left));
        type_line(&mut h, "echo LEFT-HAS-KEYS");
        wait_screen(&mut h, &probe, "the left pane", |s| {
            s.lines().any(|l| l.trim_end() == "LEFT-HAS-KEYS")
        });
        h.send(Input::chord(
            &[ShortcutKey::Super, ShortcutKey::Shift],
            ShortcutKey::Char('u'),
        ));
        wait_screen(&mut h, &probe, "the agent pane to have the keys", |s| {
            s.contains("FAKE-AGENT-WAITING")
        });
        format!("working orbs seen {worked}, then waiting=1; ⌘⇧U moved the keys to the agent pane")
    });

    steps.run("W3", || {
        // Run alone (WF_STEPS), W2's rsync has not put proj/ there.
        if !remote_home.join("proj").is_dir() {
            fs::create_dir_all(remote_home.join("proj")).expect("rhome/proj");
            for f in ["a.txt", "b.txt"] {
                fs::write(remote_home.join("proj").join(f), "x\n").expect("rhome file");
            }
        }
        let before = rows(&h);
        chord(&mut h, 't');
        type_text(&mut h, "realhost");
        h.advance(Duration::from_millis(300));
        let bar = h.text_of(".ds-palette").unwrap_or_default();
        assert!(
            bar.contains("realhost"),
            "the bar offers no realhost: {bar}"
        );
        h.send(Input::key(ShortcutKey::Enter));
        h.advance(Duration::from_millis(300));
        assert_eq!(rows(&h), before + 1, "no new tab");
        let mut answered = Vec::new();
        let yes = [
            "Set up",
            "Add it",
            "Allow",
            "Install helper",
            "Install",
            "Yes",
            "Continue",
            "Try again",
        ];
        let start = Instant::now();
        loop {
            if let Some(label) = press_yes(&mut h, ".temor-host", &yes) {
                // WF_W3_EARLY: the line typed the moment the last answer is given, before the
                // host's shell is up (the type-ahead repro; its card is lost).
                if label == "Install helper" && std::env::var_os("WF_W3_EARLY").is_some() {
                    type_line(&mut h, "ls proj");
                }
                answered.push(label);
            }
            let html = h.html();
            assert!(
                !html.contains("Couldn't connect to realhost"),
                "couldn't connect: {}; sshd.log: {}; ssh calls: {}",
                one_line(&h.text_of(".temor-host").unwrap_or_default()),
                one_line(&fs::read_to_string(host.root.join("w/sshd.log")).unwrap_or_default()),
                one_line(&fs::read_to_string(host.root.join("w/ssh-args.log")).unwrap_or_default())
            );
            if !html.contains("Connecting to realhost")
                && banner_buttons(&h, ".temor-host").is_empty()
                && start.elapsed() > Duration::from_secs(2)
            {
                break;
            }
            h.advance(Duration::from_millis(50));
            assert!(
                start.elapsed() < WAIT,
                "the host pane never came up; host view: {}; ssh calls: {}; screen: {}",
                one_line(&h.text_of(".temor-host").unwrap_or_default()),
                one_line(&fs::read_to_string(host.root.join("w/ssh-args.log")).unwrap_or_default()),
                screen_tail()
            );
        }
        let at = centre_of(first_pane(&h));
        click(&mut h, at);
        if std::env::var_os("WF_W3_EARLY").is_none() {
            // The host's shell is ready when temor's prompt (the folder, `~`) ends the screen; a
            // line typed before that runs before the pane has attached and gets no card.
            wait_screen(&mut h, &probe, "the host's first prompt", |s| {
                s.lines()
                    .rev()
                    .find(|l| !l.trim().is_empty())
                    .map(str::trim)
                    == Some("~")
            });
            type_line(&mut h, "ls proj");
        }
        wait_screen(&mut h, &probe, "the listing", |s| {
            s.contains("a.txt") && s.contains("b.txt")
        });
        wait_for(&mut h, "the ls card", |h| !metas(h).is_empty());
        format!(
            "answered {answered:?}; ls proj lists a.txt b.txt; cards {:?}",
            metas(&h)
        )
    });

    steps.run("W5", || {
        let up = host.root.join("up");
        fs::create_dir_all(&up).expect("up");
        let file = up.join("a file 'q'.txt");
        fs::write(&file, "dropped\n").expect("file");
        let at = centre_of(first_pane(&h));
        drag_files(&mut h, at, vec![file]);
        let target = remote_home.join("a file 'q'.txt");
        wait_for(&mut h, "the file on the host", |_| target.is_file());
        h.advance(Duration::from_millis(500));
        let shelf = shelf_text(&mut h);
        assert!(shelf.contains("a file 'q'.txt"), "no upload row: {shelf}");
        assert_eq!(fs::read(&target).expect("uploaded"), b"dropped\n");
        format!("rhome/a file 'q'.txt exists, same bytes; shelf {shelf}")
    });

    let mut w8_out = String::new();
    steps.run("W6", || {
        let at = centre_of(first_pane(&h));
        click(&mut h, at);
        type_line(&mut h, PRINT_REPORT);
        let mut said = Vec::new();
        // A host's permission to send files may be asked first.
        wait_for(&mut h, "the download question", |h| {
            if let Some(l) = press_yes(h, ".temor-host", &["Allow"]) {
                said.push(l);
            }
            h.html().contains("to your Downloads folder")
        });
        assert!(!home.join("Downloads/report.csv").exists(), "saved before yes");
        eprintln!("W6 question shown; resizing to 600x400 (W8)");
        h.resize_window(Extent::new(600, 400));
        h.advance(Duration::from_millis(300));
        let w8 = (|| {
            let banner = h
                .rect(".ds-inline-banner")
                .ok_or("no banner after the resize")?;
            let (sel, _) = banner_buttons(&h, "")
                .into_iter()
                .find(|(_, l)| l == "Allow")
                .ok_or("no Allow button after the resize")?;
            let button = h.rect(&sel).ok_or("no Allow rect")?;
            let inside = |r: ds::prelude::Rect| {
                r.origin.x.0 >= -0.5
                    && r.origin.y.0 >= -0.5
                    && r.origin.x.0 + r.size.width.0 <= 600.5
                    && r.origin.y.0 + r.size.height.0 <= 400.5
            };
            let fmt = |r: ds::prelude::Rect| {
                format!(
                    "({:.0},{:.0} {:.0}x{:.0})",
                    r.origin.x.0, r.origin.y.0, r.size.width.0, r.size.height.0
                )
            };
            if !inside(banner) || !inside(button) {
                return Err(format!(
                    "overflow at 600x400: banner {} Allow {}",
                    fmt(banner),
                    fmt(button)
                ));
            }
            click(&mut h, centre_of(button));
            Ok(format!("banner {} Allow {}", fmt(banner), fmt(button)))
        })();
        let saved = home.join("Downloads/report.csv");
        let start = Instant::now();
        while !saved.is_file() && start.elapsed() < Duration::from_secs(15) {
            h.advance(Duration::from_millis(25));
        }
        let w8_line = match &w8 {
            Ok(rects) if saved.is_file() => {
                format!("STEP_PASS|W8|inside 600x400: {rects}; clicking Allow's centre saved it")
            }
            Ok(rects) => format!("STEP_FAIL|W8|Allow clicked where drawn saves the file → nothing saved after 15 s; {rects}|resize to 600x400 while the download question shows, click Allow's centre"),
            Err(why) => format!("STEP_FAIL|W8|banner and Allow inside 600x400 → {why}|resize to 600x400 while the download question shows"),
        };
        w8_out.clone_from(&w8_line);
        if w8_line.starts_with("STEP_FAIL") {
            // Still answer, so W6 can be judged on its own.
            if let Some(at) = h.centre(".ds-inline-banner .ds-button") {
                click(&mut h, at);
            }
        }
        h.resize_window(Extent::new(1024, 640));
        h.advance(Duration::from_millis(300));
        wait_for(&mut h, "report.csv in Downloads", |_| saved.is_file());
        assert_eq!(fs::read(&saved).expect("saved"), b"a,b\n1,2\n");
        let shelf = shelf_text(&mut h);
        assert!(shelf.contains("report.csv"), "not on the shelf: {shelf}");
        format!("host asks answered {said:?}; Downloads {:?}; shelf {shelf}", downloads(&home))
    });

    let w6_skipped =
        std::env::var("WF_STEPS").is_ok_and(|only| !only.split(',').any(|s| s.trim() == "W6"));
    if w6_skipped {
        // W8 rides on W6's question.
    } else if w8_out.is_empty() {
        w8_out =
            "STEP_FAIL|W8|a resize while the question shows → W6 ended before the question|see W6"
                .to_owned();
    }
    if !w6_skipped {
        eprintln!("{w8_out}");
        if w8_out.starts_with("STEP_FAIL") {
            steps.failed += 1;
        }
        steps.lines.push(w8_out);
    }

    steps.run("W7", || {
        let at = centre_of(first_pane(&h));
        click(&mut h, at);
        type_line(&mut h, PRINT_REPORT);
        wait_for(&mut h, "a second report", |h| {
            if h.html().contains("to your Downloads folder") {
                let _ = press_yes(h, "", &["Allow"]);
            }
            downloads(&home)
                .iter()
                .filter(|n| n.starts_with("report"))
                .count()
                == 2
        });
        let names = downloads(&home);
        let second = names
            .iter()
            .find(|n| n.starts_with("report") && *n != "report.csv")
            .cloned()
            .unwrap_or_default();
        h.advance(Duration::from_millis(300));
        let shelf = shelf_text(&mut h);
        assert!(
            shelf.contains("report.csv") && shelf.contains(&second),
            "the shelf lacks a row: {shelf}"
        );
        assert!(
            shelf.matches("report").count() >= 2,
            "two report rows: {shelf}"
        );
        format!("new name {second:?}; Downloads {names:?}; shelf {shelf}")
    });

    steps.run("W9", || {
        let at = centre_of(first_pane(&h));
        click(&mut h, at);
        // big.bin, 1 800 000 bytes, sent slowly so the tab can close mid-way.
        type_line(
            &mut h,
            "{ printf '\\033]1337;File=name=YmlnLmJpbg==;size=1800000:'; for i in $(seq 600); do head -c 3000 /dev/zero | base64 -w0; sleep 0.05; done; printf '\\033\\\\'; }",
        );
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(4) {
            if h.html().contains("to your Downloads folder") {
                let _ = press_yes(&mut h, "", &["Allow"]);
            }
            h.advance(Duration::from_millis(50));
        }
        let during = shelf_text(&mut h);
        let tabs = rows(&h);
        let at = centre_of(first_pane(&h));
        click(&mut h, at);
        chord(&mut h, 'w');
        h.advance(Duration::from_millis(300));
        if rows(&h) == tabs {
            // A confirmation, if one is asked.
            let _ = press_yes(&mut h, "", &["Close", "Close tab", "Close anyway", "End"]);
            h.advance(Duration::from_millis(300));
        }
        assert_eq!(rows(&h), tabs - 1, "the host tab did not close");
        h.advance(Duration::from_secs(2));
        let after = shelf_text(&mut h);
        // By design (temor-pane's xfer/inline.rs, temor-vt's scan/inline_file.rs): a `File=`
        // arrives whole before anything is asked or shown, so a tab closed mid-payload leaves
        // no question, no shelf row and no file. Kitty's chunked transfer is the one with rows.
        assert!(
            !during.contains("big.bin") && !after.contains("big.bin"),
            "a row for a File= that never ended: before close {during}; after {after}"
        );
        assert!(
            !downloads(&home).iter().any(|n| n.starts_with("big")),
            "a part of big.bin was saved: {:?}",
            downloads(&home)
        );
        format!("no row before or after the close, nothing saved; shelf after {after}")
    });

    steps.run("W10", || {
        let file = home.join("Downloads/report.csv");
        assert!(file.is_file(), "no report.csv from W6");
        let before = metas(&h).len();
        let left = first_pane(&h);
        click(&mut h, centre_of(left));
        drag_files(&mut h, centre_of(left), vec![file.clone()]);
        let path = file.display().to_string();
        wait_screen(&mut h, &probe, "the dropped path at the prompt", |s| {
            s.replace('\n', "").contains(&path)
        });
        h.advance(Duration::from_secs(1));
        let screen = probe.screen();
        assert_eq!(metas(&h).len(), before, "something ran");
        assert!(
            !screen.contains("Permission denied") && !screen.contains("command not found"),
            "the path ran: {screen}"
        );
        // Leave the line unrun.
        h.send(Input::chord(&[ShortcutKey::Ctrl], ShortcutKey::Char('u')));
        format!("path {path} typed at the prompt; cards stay {before}")
    });

    drop(h);
    drop(host);
    eprintln!("sshd stopped");
    let report = steps.lines.join("\n");
    assert_eq!(steps.failed, 0, "steps failed:\n{report}");
}
