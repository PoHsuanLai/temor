//! What a session and a connection promise while the host is busy or the terminal is sized in
//! pixels: a slow file request leaves the link answering, and the pixel size the program reads
//! back is the one the window gave.

use std::time::{Duration, Instant};

use temor_wire::message::RemotePath;
use temor_wire::units::{CellPx, Cols, Pixels, Rows, Size};

use crate::support::{Rig, tools_present};

/// Large enough that hashing it for a stamp takes seconds, while the file itself takes no room
/// (a sparse file of zeros).
const SPARSE_LEN: u64 = 4 << 30;

#[test]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "confined file access is Linux-only so far"
)]
fn a_slow_file_request_does_not_make_the_link_look_dead() {
    let rig = Rig::new("live-slow");
    let big = rig.files.join("big.bin");
    let file = std::fs::File::create(&big).expect("file");
    file.set_len(SPARSE_LEN).expect("a sparse file");
    drop(file);
    let remote = RemotePath(big.display().to_string());
    let conn = rig.connect();

    let (stat, stat_done, pong) = std::thread::scope(|scope| {
        let slow = scope.spawn(|| {
            let stat = conn.client.stat(&remote);
            (stat, Instant::now())
        });
        // Let the request reach the file worker before the ping follows it.
        std::thread::sleep(Duration::from_millis(200));
        let pong = conn
            .client
            .ping(Duration::from_secs(5))
            .map(|()| Instant::now());
        let (stat, stat_done) = slow.join().expect("the stat thread did not panic");
        (stat, stat_done, pong)
    });
    stat.expect("the slow stat still answers");
    let pong = pong.expect("the ping is answered while the file is hashed");
    assert!(
        pong < stat_done,
        "the ping waited for the file request to finish"
    );
}

fn read_back(rig: &Rig, size: Size, cell: CellPx) -> String {
    let mut request = rig.bash();
    request.size = size;
    request.cell = cell;
    let mut conn = rig.connect();
    let id = conn.client.open_session(request).expect("open");
    conn.client.attach(id).expect("attach");
    // The terminal answers a text-area size question with `ESC [ 4 ; height ; width t`; the
    // shell reads up to the `t` and prints what came between the bracket and it.
    let ask = b"printf '\\033[14t'; IFS= read -r -s -d t r; echo \"px:${r#*[}:end\"\n";
    conn.client.send_input(id, ask).expect("input");
    let out = conn
        .watch
        .output_has("the pixel size", |out| out.contains(":end\r\n"));
    let start = out.rfind("px:4;").expect("the answer");
    out[start..]
        .lines()
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_string()
}

#[test]
fn the_program_reads_back_the_pixel_size_the_window_gave() {
    if !tools_present() {
        return;
    }
    let rig = Rig::new("live-pixels");
    let size = |cols, rows| Size {
        cols: Cols(cols),
        rows: Rows(rows),
    };
    let cell = |width, height| CellPx {
        width: Pixels(width),
        height: Pixels(height),
    };
    // Size, cell, and the text area in pixels the program should read (height, then width).
    let cases = [
        (size(80, 24), cell(10, 20), "px:4;480;800:end"),
        (size(100, 30), cell(9, 18), "px:4;540;900:end"),
        // A cell of no size is read as the default.
        (size(80, 24), cell(0, 20), "px:4;384;640:end"),
    ];
    for (size, cell, want) in cases {
        assert_eq!(read_back(&rig, size, cell), want, "{size:?} {cell:?}");
    }
}
