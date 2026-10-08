//! Screen behaviour: reflow on resize, scrollback and the viewport, dirty rows across two-phase
//! updates, synchronized output (`sync.bin`) and text export.

use temor_vt::{
    CellPx, FrameDirty, GridPoint, GridSize, ModeState, PointSpace, RenderHold, RenderView,
    RowCount, RowDelta, RowFilter, TerminalMode, TextFormat, TextSpan, ViewportScroll, VtEvent,
};

use crate::support::{drain, fixture, row_texts, term};

fn plain(t: &temor_vt::Terminal) -> String {
    temor_vt::Formatter::new(TextFormat::Plain)
        .format(t, TextSpan::Screen)
        .expect("text")
}

#[test]
fn resize_reflows_soft_wrapped_lines() {
    let (mut t, _rx) = term(80, 24);
    let line = fixture("reflow.bin");
    t.write(&line);
    let text = String::from_utf8(line[..150].to_vec()).expect("ascii");
    assert_eq!(row_texts(&mut t, 1).concat(), text[80..]);
    t.resize(GridSize::new(160, 24), CellPx::new(8, 16))
        .expect("resize");
    assert_eq!(row_texts(&mut t, 0).concat(), text);
    t.resize(GridSize::new(50, 24), CellPx::new(8, 16))
        .expect("resize");
    assert_eq!(row_texts(&mut t, 2).concat(), text[100..]);
    assert_eq!(plain(&t), text);
}

#[test]
fn viewport_scrolls_over_scrollback() {
    let (mut t, _rx) = term(20, 4);
    for i in 0..30 {
        t.write(format!("line {i}\r\n").as_bytes());
    }
    let bottom = t.scrollbar().expect("scrollbar");
    assert_eq!(bottom.len, RowCount(4));
    assert_eq!(bottom.offset.0 + bottom.len.0, bottom.total.0);
    t.scroll_viewport(ViewportScroll::Top);
    assert_eq!(t.scrollbar().expect("scrollbar").offset, RowCount(0));
    assert_eq!(row_texts(&mut t, 0).concat(), "line 0");
    t.scroll_viewport(ViewportScroll::Delta(RowDelta(2)));
    assert_eq!(row_texts(&mut t, 0).concat(), "line 2");
    t.scroll_viewport(ViewportScroll::Bottom);
    assert_eq!(t.scrollbar().expect("scrollbar"), bottom);
}

#[test]
fn two_phase_update_reports_only_changed_rows() {
    let (mut t, _rx) = term(20, 6);
    t.write(b"one\r\ntwo\r\nthree");
    let mut view = RenderView::new().expect("view");
    let frame = view.begin(&mut t).expect("begin").end().expect("end");
    assert_eq!(frame.dirty(), FrameDirty::Full);
    frame.mark_drawn().expect("drawn");

    let frame = view.begin(&mut t).expect("begin").end().expect("end");
    assert_eq!(frame.dirty(), FrameDirty::Clean);
    frame.mark_drawn().expect("drawn");

    t.write(b"\x1b[5;1Hfive");
    let mut frame = view.begin(&mut t).expect("begin").end().expect("end");
    assert_eq!(frame.dirty(), FrameDirty::Partial);
    let mut dirty = Vec::new();
    frame
        .rows(RowFilter::Dirty, |r| dirty.push(r.row()))
        .expect("rows");
    assert!(dirty.contains(&4), "the edited row is dirty: {dirty:?}");
    assert!(!dirty.contains(&0), "untouched rows stay clean: {dirty:?}");
    let cursor = frame.cursor().expect("cursor shown");
    assert_eq!((cursor.col.0, cursor.row), (4, 4));
}

#[test]
fn synchronized_output_holds_and_releases_rendering() {
    let (mut t, rx) = term(20, 4);
    t.write(b"\x1b[?2026h");
    assert_eq!(t.mode(TerminalMode::SyncOutput), Ok(ModeState::Set));
    let mut sync = fixture("sync.bin");
    sync.drain(..8);
    t.write(&sync);
    assert_eq!(t.mode(TerminalMode::SyncOutput), Ok(ModeState::Reset));
    let holds: Vec<RenderHold> = drain(&rx)
        .into_iter()
        .filter_map(|e| match e {
            VtEvent::RenderHold(h) => Some(h),
            _ => None,
        })
        .collect();
    assert_eq!(holds, [RenderHold::Held, RenderHold::Released]);
}

#[test]
fn ranges_export_as_plain_and_html() {
    let (mut t, _rx) = term(40, 4);
    t.write(b"plain \x1b[31mred\x1b[0m\r\nsecond");
    let range = TextSpan::Range {
        start: GridPoint::new(PointSpace::Active, 6, 0),
        end: GridPoint::new(PointSpace::Active, 2, 1),
    };
    let text = temor_vt::Formatter::new(TextFormat::Plain)
        .format(&t, range)
        .expect("plain");
    assert_eq!(text, "red\nsec");
    let html = temor_vt::Formatter::new(TextFormat::Html)
        .format(&t, range)
        .expect("html");
    assert!(
        html.contains("red") && html.contains("--vt-palette-1"),
        "{html}"
    );
}
