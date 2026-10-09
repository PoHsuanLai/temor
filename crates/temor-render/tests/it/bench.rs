//! Frame time, capture plus CPU render, while scrolling one line per frame. Ignored by default:
//! `cargo test --release -p temor-render -- --ignored --nocapture frame_time`.

use std::time::{Duration, Instant};

use temor_render::PaneFocus;

use crate::support::Pane;

fn measure(cols: u16, rows: u16) {
    let mut pane = Pane::new(cols, rows, 2.0, 13.0);
    let line: Vec<u8> = (0..cols)
        .map(|i| b"abcdefghijklmnopqrstuvwxyz0123456789"[usize::from(i) % 36])
        .collect();
    for _ in 0..rows {
        pane.write(&line);
        pane.write(b"\r\n");
    }
    pane.render(PaneFocus::Focused);
    let mut times: Vec<Duration> = Vec::new();
    for i in 0..240u32 {
        pane.write(format!("\x1b[3{}m", i % 8).as_bytes());
        pane.write(&line);
        pane.write(b"\r\n");
        let start = Instant::now();
        let shot = pane.render(PaneFocus::Focused);
        times.push(start.elapsed());
        assert!(shot.stats.rows_composed > 0);
    }
    times.sort();
    let p = |q: f64| times[((times.len() - 1) as f64 * q) as usize];
    println!(
        "{cols}x{rows} at 2x ({}x{} px): p50 {:?}, p95 {:?}, max {:?}",
        pane.size.width.0,
        pane.size.height.0,
        p(0.5),
        p(0.95),
        p(1.0)
    );
}

#[test]
#[ignore = "benchmark; run with --ignored --nocapture"]
fn frame_time() {
    measure(80, 24);
    measure(200, 60);
}
