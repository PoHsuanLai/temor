//! A link with a fixed one-way delay and a byte rate, between a client and the daemon, as a far
//! host over a thin line would be: what the person feels when typing while a file moves.

use std::thread;
use std::time::{Duration, Instant};

use temor_remote::Client;
use temor_wire::ids::SessionId;
use temor_wire::message::{RemotePath, WriteCheck};

use crate::pipes::Line;
use crate::support::{Conn, Rig, tools_present};

/// How long a typed line takes to come back on the screen.
fn echo_after(conn: &mut Conn, id: SessionId, n: u32) -> Duration {
    let start = Instant::now();
    conn.client
        .send_input(id, format!("echo mark{n}\n").as_bytes())
        .expect("input");
    let want = format!("mark{n}\r\n");
    conn.watch.output_has("the echo", |out| out.contains(&want));
    start.elapsed()
}

/// Times three echoes while `work` runs on the same connection, and how long `work` took.
fn echoes_during(
    conn: &mut Conn,
    id: SessionId,
    first: u32,
    work: impl FnOnce(&Client) + Send,
) -> (Vec<Duration>, Duration) {
    let Conn { client, watch, .. } = conn;
    let client: &Client = client;
    thread::scope(|scope| {
        let busy = scope.spawn(move || {
            let start = Instant::now();
            work(client);
            start.elapsed()
        });
        thread::sleep(Duration::from_millis(500));
        let mut echoes = Vec::new();
        for n in first..first + 3 {
            let start = Instant::now();
            client
                .send_input(id, format!("echo mark{n}\n").as_bytes())
                .expect("input");
            let want = format!("mark{n}\r\n");
            watch.output_has("the echo", |out| out.contains(&want));
            echoes.push(start.elapsed());
        }
        (echoes, busy.join().expect("the busy thread"))
    })
}

#[test]
#[ignore = "a measurement: by hand, `-- --ignored --nocapture`"]
fn typing_while_a_file_moves_over_a_thin_line() {
    if !tools_present() {
        return;
    }
    let line = Line {
        one_way: Duration::from_millis(75),
        bytes_per_sec: 1_000_000,
    };
    let rig = Rig::new("slow-link");
    let big = rig.files.join("big.bin");
    std::fs::write(&big, vec![7u8; 8 << 20]).expect("file");
    let start = Instant::now();
    let mut conn = rig.connect_over(line);
    eprintln!("connect: {:?}", start.elapsed());
    let id = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(id).expect("attach");
    eprintln!("idle echo: {:?}", echo_after(&mut conn, id, 1));
    let remote = RemotePath(big.display().to_string());
    let (echoes, took) = echoes_during(&mut conn, id, 2, |client| {
        client.read_all(&remote).expect("read");
    });
    eprintln!("8 MiB read took {took:?}; echoes meanwhile {echoes:?}");
    let up = rig.home.join("up.bin");
    std::fs::write(&up, vec![9u8; 8 << 20]).expect("file");
    let target = RemotePath(rig.files.join("up.bin").display().to_string());
    let alone = rig.files.join("alone.bin");
    let start = Instant::now();
    conn.client
        .upload(
            &up,
            &RemotePath(alone.display().to_string()),
            WriteCheck::MustBeNew,
        )
        .expect("upload alone");
    eprintln!("8 MiB upload alone took {:?}", start.elapsed());
    let (echoes, took) = echoes_during(&mut conn, id, 5, |client| {
        client
            .upload(&up, &target, WriteCheck::MustBeNew)
            .expect("upload");
    });
    eprintln!("8 MiB upload took {took:?}; echoes meanwhile {echoes:?}");
}

#[test]
#[ignore = "a measurement: by hand"]
fn upload_over_plain_pipes() {
    let rig = Rig::new("plain-up");
    let up = rig.home.join("up.bin");
    std::fs::write(&up, vec![9u8; 8 << 20]).expect("file");
    let conn = rig.connect();
    for n in 0..3 {
        let start = Instant::now();
        let to = RemotePath(rig.files.join(format!("up{n}.bin")).display().to_string());
        conn.client
            .upload(&up, &to, WriteCheck::MustBeNew)
            .expect("upload");
        eprintln!("plain upload {n}: {:?}", start.elapsed());
    }
}

#[test]
fn a_keystroke_is_not_held_behind_an_upload_on_a_thin_line() {
    if !tools_present() {
        return;
    }
    // 1 MB/s and 150 ms there and back. A part in flight ahead of the keystroke costs a quarter
    // of a second; one 4 MiB part, as uploads once went, cost four.
    let line = Line {
        one_way: Duration::from_millis(75),
        bytes_per_sec: 1_000_000,
    };
    let rig = Rig::new("slow-link-keys");
    let up = rig.home.join("up.bin");
    std::fs::write(&up, vec![9u8; 4 << 20]).expect("file");
    let target = RemotePath(rig.files.join("up.bin").display().to_string());
    let mut conn = rig.connect_over(line);
    let id = conn.client.open_session(rig.bash()).expect("open");
    conn.client.attach(id).expect("attach");
    let (echoes, _) = echoes_during(&mut conn, id, 1, |client| {
        client
            .upload(&up, &target, WriteCheck::MustBeNew)
            .expect("upload");
    });
    let worst = echoes.iter().max().copied().unwrap_or_default();
    assert!(
        worst < Duration::from_secs(2),
        "a keystroke waited {worst:?} behind the upload ({echoes:?})"
    );
}
