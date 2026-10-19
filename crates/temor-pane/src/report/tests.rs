use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use super::protocol::{Request, parse_request};
use super::{
    PaneKey, PaneSocket, ReportRefusal, ReportState, ReportWait, SocketLimits, StatusLine,
    encode_hello, encode_status, parse_reply,
};

fn status(seq: u64, state: ReportState) -> StatusLine {
    StatusLine {
        seq,
        state,
        wait: None,
        app: None,
        note: None,
    }
}

#[test]
fn request_lines_parse_or_are_refused() {
    let waiting = StatusLine {
        wait: Some(ReportWait::Permission),
        app: Some("claude".into()),
        note: Some("Run it?\u{7}".into()),
        ..status(7, ReportState::Waiting)
    };
    let expected_note = StatusLine {
        note: Some("Run it?".into()),
        ..waiting.clone()
    };
    #[rustfmt::skip]
    let cases: Vec<(&str, String, Result<Request, ReportRefusal>)> = vec![
        ("hello", "hello 1".into(), Ok(Request::Hello(1))),
        ("hello without a number", "hello".into(), Err(ReportRefusal::Syntax)),
        ("ping", "ping".into(), Ok(Request::Ping)),
        ("a plain status", "status 3 working".into(), Ok(Request::Status(status(3, ReportState::Working)))),
        ("wait only counts for waiting", "status 3 done wait=question".into(), Ok(Request::Status(status(3, ReportState::Done)))),
        ("an unknown key is ignored", "status 3 idle future=1".into(), Ok(Request::Status(status(3, ReportState::Idle)))),
        ("a full status, note decoded and cleaned", encode_status(&waiting).trim_end().to_owned(), Ok(Request::Status(expected_note))),
        ("a bad state", "status 3 sleeping".into(), Err(ReportRefusal::Syntax)),
        ("a bad seq", "status x idle".into(), Err(ReportRefusal::Syntax)),
        ("a bad wait", "status 1 waiting wait=sulking".into(), Err(ReportRefusal::Syntax)),
        ("a broken escape", "status 1 idle note=%G1".into(), Err(ReportRefusal::Syntax)),
        ("a note that is not utf-8", "status 1 idle note=%FF".into(), Err(ReportRefusal::Syntax)),
        ("a note over its limit", format!("status 1 idle note={}", "a".repeat(513)), Err(ReportRefusal::TooLong)),
        ("a line over its limit", format!("status 1 idle note={}", "a".repeat(5000)), Err(ReportRefusal::TooLong)),
        ("a stranger", "rm -rf /".into(), Err(ReportRefusal::Syntax)),
    ];
    for (name, line, want) in cases {
        assert_eq!(parse_request(&line), want, "{name}");
    }
}

fn client(socket: &PaneSocket) -> UnixStream {
    let stream = UnixStream::connect(socket.path()).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    stream
}

fn read_some(stream: &mut UnixStream) -> String {
    let mut buf = [0u8; 256];
    let n = stream.read(&mut buf).expect("a reply");
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "the private report socket is Linux-only so far"
)]
fn a_client_reports_through_the_pump_and_bad_input_is_answered() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = PaneKey::new("p-12").expect("key");
    let mut socket = PaneSocket::bind(dir.path(), key, SocketLimits::default()).expect("bind");
    let mode = std::fs::metadata(socket.path())
        .expect("meta")
        .permissions()
        .mode();
    assert_eq!(mode & 0o077, 0, "the socket is private");

    // Status before hello is refused but the connection stays.
    let mut a = client(&socket);
    let report = StatusLine {
        app: Some("claude".into()),
        ..status(1, ReportState::Working)
    };
    let script = format!(
        "{}{}{}",
        encode_status(&report),
        encode_hello(),
        encode_status(&report)
    );
    a.write_all(script.as_bytes()).expect("write");
    assert_eq!(socket.pump(), vec![report.clone()]);
    assert_eq!(read_some(&mut a), "err need_hello\nok 1\nok 1\n");
    assert_eq!(parse_reply("ok 1\n"), Ok(()));

    // A second pump with nothing sent returns nothing; ping answers.
    a.write_all(b"ping\n").expect("write");
    assert!(socket.pump().is_empty());
    assert_eq!(read_some(&mut a), "pong\n");

    // A wrong version ends the connection.
    let mut b = client(&socket);
    b.write_all(b"hello 9\n").expect("write");
    socket.pump();
    assert_eq!(read_some(&mut b), "err version\n");
    assert_eq!(parse_reply("err version"), Err(ReportRefusal::Version));
    assert_eq!(b.read(&mut [0u8; 8]).expect("eof"), 0);

    // A line that never ends is cut off.
    let mut c = client(&socket);
    c.write_all(&vec![b'x'; 5000]).expect("write");
    socket.pump();
    assert_eq!(read_some(&mut c), "err too_long\n");

    let path = socket.path().to_path_buf();
    drop(socket);
    assert!(!path.exists(), "dropping the socket removes its file");
}

#[test]
fn pane_keys_cannot_leave_the_directory() {
    let bad = [
        String::new(),
        "../x".into(),
        "a/b".into(),
        "a b".into(),
        "x".repeat(33),
    ];
    for bad in bad {
        assert!(PaneKey::new(&bad).is_err(), "{bad:?}");
    }
    assert!(PaneKey::new("pane_7-a").is_ok());
}
