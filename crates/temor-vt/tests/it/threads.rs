//! The P0(a) threading pattern: an I/O thread writes under the lock while a render thread runs
//! `begin` under the lock and `end` outside it.

use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use temor_vt::{GridSize, RenderView, RowFilter, Terminal, TerminalConfig, VtEvent};

const WRITES: usize = 2000;
const FRAMES: usize = 300;

#[test]
fn concurrent_writes_and_renders_lose_nothing() {
    let (tx, _events) = mpsc::channel::<VtEvent>();
    let (reply_tx, rx) = mpsc::channel::<Vec<u8>>();
    let config = TerminalConfig::new(GridSize::new(80, 24));
    let term = Terminal::new(config, Box::new(tx), Box::new(reply_tx)).expect("terminal");
    let term = Arc::new(Mutex::new(term));

    let writer = {
        let term = Arc::clone(&term);
        thread::spawn(move || {
            temor_vt::install_png_decoder();
            for i in 0..WRITES {
                let mut t = term.lock().expect("lock");
                t.write(format!("\x1b[5nline {i}\r\n").as_bytes());
            }
        })
    };
    let renderer = {
        let term = Arc::clone(&term);
        thread::spawn(move || {
            let mut view = RenderView::new().expect("view");
            let mut rows_seen = 0usize;
            for _ in 0..FRAMES {
                let pending = {
                    let mut t = term.lock().expect("lock");
                    view.begin(&mut t).expect("begin")
                };
                let mut frame = pending.end().expect("end");
                frame
                    .rows(RowFilter::Dirty, |r| r.cells(|_| {}).expect("cells"))
                    .expect("rows");
                rows_seen += 1;
                frame.mark_drawn().expect("drawn");
            }
            rows_seen
        })
    };
    writer.join().expect("writer");
    assert_eq!(renderer.join().expect("renderer"), FRAMES);

    let replies = rx.try_iter().filter(|r| r.as_slice() == b"\x1b[0n").count();
    assert_eq!(replies, WRITES);
    let mut t = term.lock().expect("lock");
    assert_eq!(
        crate::support::row_texts(&mut t, 22).concat(),
        format!("line {}", WRITES - 1)
    );
}
