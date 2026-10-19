//! A real sshd on 127.0.0.1, run as this user with a scratch host key, authorized key and
//! known_hosts. Its forced command swaps in a scratch HOME, so nothing in the real home is
//! written. temor's own argv builders and master probe drive it through an `ssh` wrapper that
//! adds `-F` with the scratch client config (ssh finds `~/.ssh` through the password database,
//! not `$HOME`). Skipped where sshd, ssh or ssh-keygen is missing.

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use temor_ssh::{
    CommandRunner, ControlLayout, ControlPersist, FileTools, Forward, HostAlias, MasterProbe, Port,
    RemoteHost, RuntimeDir, StdRunner, cancel_forward_argv, exit_argv, forward_argv,
    prepare_control_dirs, probe_master, session_argv,
};

use crate::harness::{Scratch, remote, require, scratch, write};

/// A running sshd and the client side that reaches it as `realhost`.
struct Host {
    s: Scratch,
    sshd: Child,
    /// The `ssh` wrapper temor runs.
    tools: FileTools,
    alias: HostAlias,
    layout: ControlLayout,
    /// Holds the control sockets: a short path, as a socket's path is limited to ~100 bytes.
    _sockets: tempfile::TempDir,
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = StdRunner.run(&self.tools.ssh, &exit_argv(&self.alias, &self.layout));
        let _ = self.sshd.kill();
        let _ = self.sshd.wait();
    }
}

fn keygen(keygen: &Path, file: &Path) {
    let status = Command::new(keygen)
        .args(["-q", "-t", "ed25519", "-N", "", "-f"])
        .arg(file)
        .status()
        .expect("ssh-keygen");
    assert!(status.success());
}

fn free_port() -> u16 {
    let l = TcpListener::bind(("127.0.0.1", 0)).expect("bind");
    l.local_addr().expect("addr").port()
}

fn executable(path: &Path, text: &str) {
    write(path, text);
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod");
}

/// Starts sshd, or `None` (a skip) where a tool is missing.
fn host() -> Option<Host> {
    let (sshd, ssh, ssh_keygen) = (require("sshd")?, require("ssh")?, require("ssh-keygen")?);
    let bash = require("bash")?;
    let bash = bash.display();
    let s = scratch();
    let w = &s.work;
    keygen(&ssh_keygen, &w.join("host_key"));
    keygen(&ssh_keygen, &w.join("client_key"));
    fs::copy(w.join("client_key.pub"), w.join("authorized_keys")).expect("authorized");
    let port = free_port();
    let user = std::env::var("USER").expect("USER");
    let (home, tmp) = (s.home.display(), s.tmp.display());
    executable(
        &w.join("force.sh"),
        &format!(
            "#!/bin/sh\n\
             export HOME={home} TMPDIR={tmp} HISTFILE=/dev/null SHELL={bash}\n\
             cd \"$HOME\" || exit 70\n\
             [ -n \"$SSH_ORIGINAL_COMMAND\" ] || exit 3\n\
             exec /bin/sh -c \"$SSH_ORIGINAL_COMMAND\"\n"
        ),
    );
    let wd = w.display();
    write(
        &w.join("sshd_config"),
        &format!(
            "ListenAddress 127.0.0.1\nPort {port}\nHostKey {wd}/host_key\n\
             AuthorizedKeysFile {wd}/authorized_keys\nPidFile none\nUsePAM no\nStrictModes no\n\
             PasswordAuthentication no\nKbdInteractiveAuthentication no\nPermitUserRC no\n\
             PermitUserEnvironment no\nAllowTcpForwarding local\nForceCommand {wd}/force.sh\n"
        ),
    );
    write(
        &w.join("ssh_config"),
        &format!(
            "Host realhost\n  HostName 127.0.0.1\n  Port {port}\n  User {user}\n\
             IdentityFile {wd}/client_key\n  IdentitiesOnly yes\n\
             UserKnownHostsFile {wd}/known_hosts\n  GlobalKnownHostsFile /dev/null\n\
             StrictHostKeyChecking accept-new\n  BatchMode yes\n  LogLevel ERROR\n"
        ),
    );
    let wrapper = s.bin.join("ssh");
    executable(
        &wrapper,
        &format!(
            "#!/bin/sh\nexec {} -F {wd}/ssh_config \"$@\"\n",
            ssh.display()
        ),
    );
    let log = fs::File::create(w.join("sshd.log")).expect("log");
    let child = Command::new(sshd)
        .args(["-D", "-e", "-f"])
        .arg(w.join("sshd_config"))
        .stdin(Stdio::null())
        .stdout(log.try_clone().expect("log"))
        .stderr(log)
        .spawn()
        .expect("sshd");
    let up = Instant::now();
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            up.elapsed() < Duration::from_secs(10),
            "sshd never listened"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let sockets = tempfile::Builder::new()
        .prefix("ts")
        .tempdir_in("/tmp")
        .expect("socket dir");
    let layout = ControlLayout::new(
        RuntimeDir::new(&sockets.path().join("t")).expect("runtime"),
        ControlPersist::Forever,
    )
    .expect("layout");
    prepare_control_dirs(&layout).expect("control dirs");
    Some(Host {
        sshd: child,
        tools: FileTools {
            ssh: wrapper,
            scp: PathBuf::from("scp"),
        },
        alias: HostAlias::new("realhost").expect("alias"),
        layout,
        s,
        _sockets: sockets,
    })
}

impl Host {
    /// A session as a pane starts one: the bootstrap on the shared connection, with `exit`
    /// typed. Its output goes to a file, as the master it leaves behind keeps the stream open.
    fn session(&self) -> (bool, String) {
        let out = self.s.work.join("session.out");
        let mut child = Command::new(&self.tools.ssh)
            .args(session_argv(&self.alias, &self.layout, &remote()))
            .stdin(Stdio::piped())
            .stdout(fs::File::create(&out).expect("out"))
            .stderr(fs::File::create(self.s.work.join("session.err")).expect("err"))
            .spawn()
            .expect("ssh");
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(b"exit\n");
        }
        let ok = child.wait().expect("wait").success();
        let text = fs::read_to_string(&out).unwrap_or_default();
        let err = fs::read_to_string(self.s.work.join("session.err")).unwrap_or_default();
        (ok, format!("{text}{err}"))
    }

    fn probe(&self) -> MasterProbe {
        probe_master(&StdRunner, &self.tools, &self.alias, &self.layout).expect("probe")
    }

    fn run(&self, argv: &[String]) -> (Option<i32>, String) {
        let out = StdRunner.run(&self.tools.ssh, argv).expect("ssh");
        (out.code, String::from_utf8_lossy(&out.stderr).into_owned())
    }

    fn forward(&self, local: u16, remote: u16) -> Forward {
        Forward {
            bind: None,
            local: Port::parse(&local.to_string()).expect("port"),
            remote_host: RemoteHost::localhost(),
            remote: Port::parse(&remote.to_string()).expect("port"),
        }
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "the private runtime folder is Linux-only so far"
)]
fn the_bootstrap_runs_on_a_real_host_and_leaves_a_shared_connection() {
    let Some(host) = host() else {
        return;
    };
    assert_eq!(host.probe(), MasterProbe::NotRunning);
    let before = host.layout.finished_sockets();
    let (ok, text) = host.session();
    assert!(ok, "the session failed:\n{text}");
    assert!(text.contains("\x1b]133;A"), "no prompt mark:\n{text:?}");
    assert!(temor_ssh::socket_appeared(
        &before,
        &host.layout.finished_sockets()
    ));
    assert_eq!(host.probe(), MasterProbe::Running);
    // A second session joins the master without logging in again.
    let (ok, text) = host.session();
    assert!(ok && text.contains("\x1b]133;A"), "{text:?}");
    let (code, said) = host.run(&exit_argv(&host.alias, &host.layout));
    assert_eq!(code, Some(0), "{said}");
    let gone = Instant::now();
    while host.probe() == MasterProbe::Running {
        assert!(gone.elapsed() < Duration::from_secs(5), "the master stayed");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "the private runtime folder is Linux-only so far"
)]
fn forwards_open_carry_close_and_fail_with_the_words_temor_reads() {
    let Some(host) = host() else {
        return;
    };
    let service = TcpListener::bind(("127.0.0.1", 0)).expect("service");
    let remote_port = service.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        for stream in service.incoming().flatten() {
            let mut stream = stream;
            let _ = stream.write_all(b"hello from the host");
        }
    });
    let local = free_port();
    let fwd = host.forward(local, remote_port);

    // No connection yet: ssh says its control socket is missing.
    let (code, said) = host.run(&forward_argv(&host.alias, &host.layout, &fwd));
    assert_ne!(code, Some(0));
    assert!(said.to_lowercase().contains("control socket"), "{said}");

    let (ok, text) = host.session();
    assert!(ok, "{text}");
    let (code, said) = host.run(&forward_argv(&host.alias, &host.layout, &fwd));
    assert_eq!(code, Some(0), "{said}");
    let mut got = String::new();
    TcpStream::connect(("127.0.0.1", local))
        .expect("the forward listens")
        .read_to_string(&mut got)
        .expect("read");
    assert_eq!(got, "hello from the host");

    // Asking again for the same forward is no error: the master already has it.
    let (code, said) = host.run(&forward_argv(&host.alias, &host.layout, &fwd));
    assert_eq!(code, Some(0), "{said}");

    // A port another program holds on both loopbacks: the master says forwarding failed.
    let v4 = TcpListener::bind(("127.0.0.1", 0)).expect("held");
    let held = v4.local_addr().expect("addr").port();
    let _v6 = TcpListener::bind(("::1", held));
    let (code, said) = host.run(&forward_argv(
        &host.alias,
        &host.layout,
        &host.forward(held, remote_port),
    ));
    assert_ne!(code, Some(0));
    assert!(
        said.to_lowercase().contains("port forwarding failed"),
        "{said}"
    );

    let (code, said) = host.run(&cancel_forward_argv(&host.alias, &host.layout, &fwd));
    assert_eq!(code, Some(0), "{said}");
    let closed = Instant::now();
    while TcpStream::connect(("127.0.0.1", local)).is_ok() {
        assert!(
            closed.elapsed() < Duration::from_secs(5),
            "the forward stayed"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}
