//! A size audit, run on demand (`--ignored`): the window in its main states, and for every element
//! on screen its box, padding, type and corner radius, written as JSON lines to
//! `$TEMOR_SIZE_AUDIT_OUT` (or the test's scratch folder). Nothing is asserted; the dump is read by
//! a person or a script.

use std::fmt::Write as _;
use std::time::Duration;

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_blitz::Extent;
use ds_harness::{Harness, Input};
use temor_ui::view::probe::Probe;

use super::support::scratch_path;
use super::window::{harness, wait_until};

fn chord(h: &mut Harness, key: char, shift: bool) {
    let held: &[ShortcutKey] = match shift {
        true => &[ShortcutKey::Ctrl, ShortcutKey::Shift],
        false => &[ShortcutKey::Ctrl],
    };
    h.send(Input::chord(held, ShortcutKey::Char(key)));
    h.advance(Duration::from_millis(400));
}

fn typed(h: &mut Harness, text: &str) {
    for c in text.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    h.advance(Duration::from_millis(300));
}

fn esc(v: &str) -> String {
    v.chars()
        .take(48)
        .flat_map(|c| match c {
            '"' => vec!['\\', '"'],
            '\\' => vec!['\\', '\\'],
            c if c.is_control() => vec![' '],
            c => vec![c],
        })
        .collect()
}

/// Every element with a box, as JSON lines tagged with `state`.
fn dump(h: &Harness, state: &str, out: &mut String) {
    h.with_doc(|doc| {
        let mut stack = vec![doc.root_node().id];
        while let Some(id) = stack.pop() {
            let Some(node) = doc.get_node(id) else {
                continue;
            };
            stack.extend(node.children.iter().rev().copied());
            let Some(el) = node.element_data() else {
                continue;
            };
            let Some(rect) = doc.get_client_bounding_rect(id) else {
                continue;
            };
            if rect.width < 0.5 || rect.height < 0.5 {
                continue;
            }
            let attr = |name: &str| {
                el.attrs()
                    .iter()
                    .find(|a| &*a.name.local == name)
                    .map(|a| a.value.clone())
                    .unwrap_or_default()
            };
            let (size, weight, radius) = match node.primary_styles() {
                Some(s) => {
                    let font = s.get_font();
                    let radius = s
                        .get_border()
                        .border_top_left_radius
                        .0
                        .width
                        .0
                        .to_length()
                        .map_or(-1.0, |l| l.px());
                    (
                        font.font_size.computed_size().px(),
                        font.font_weight.value(),
                        radius,
                    )
                }
                None => (-1.0, -1.0, -1.0),
            };
            let l = node.final_layout();
            let text = match node.children.len() {
                0..=2 => node.text_content(),
                _ => String::new(),
            };
            let _ = writeln!(
                out,
                "{{\"state\":\"{state}\",\"tag\":\"{}\",\"class\":\"{}\",\"label\":\"{}\",\"x\":{:.1},\"y\":{:.1},\"w\":{:.1},\"h\":{:.1},\"pad\":[{:.1},{:.1},{:.1},{:.1}],\"border\":{:.1},\"font\":{size:.2},\"weight\":{weight:.0},\"radius\":{radius:.1},\"text\":\"{}\"}}",
                &*el.name.local,
                esc(&attr("class")),
                esc(&format!("{}{}", attr("aria-label"), attr("title"))),
                rect.x,
                rect.y,
                rect.width,
                rect.height,
                l.padding.left,
                l.padding.top,
                l.padding.right,
                l.padding.bottom,
                l.border.top,
                esc(text.trim()),
            );
        }
    });
}

#[test]
#[ignore = "a size audit, run on demand"]
fn size_audit() {
    if !super::support::tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness("size-audit", &probe);
    h.resize_window(Extent::new(1280, 800));
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(700.0),
        y: Px(400.0),
    }));
    h.advance(Duration::from_millis(100));
    let mut out = String::new();
    dump(&h, "start", &mut out);
    typed(&mut h, "echo one");
    typed(&mut h, "ls /nonexistent-file");
    typed(&mut h, "seq 1 3");
    wait_until(&mut h, &probe, "the output", |s| {
        s.contains("3\n") || s.ends_with('3')
    });
    dump(&h, "cards", &mut out);
    // The pointer over the first card brings its tools.
    h.send(Input::pointer_move(Point {
        x: Px(900.0),
        y: Px(40.0),
    }));
    h.advance(Duration::from_millis(300));
    dump(&h, "hover", &mut out);
    chord(&mut h, 't', true);
    dump(&h, "bar", &mut out);
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(200));
    chord(&mut h, 'f', true);
    dump(&h, "find", &mut out);
    h.send(Input::key(ShortcutKey::Escape));
    h.advance(Duration::from_millis(200));
    chord(&mut h, 'n', true);
    chord(&mut h, 'n', true);
    dump(&h, "tabs", &mut out);
    chord(&mut h, 'd', true);
    dump(&h, "split", &mut out);
    if let Some(row) = h.centre(".ds-row") {
        h.send(Input::press(row, PointerButton::Secondary));
        h.advance(Duration::from_millis(300));
        dump(&h, "menu", &mut out);
        h.send(Input::key(ShortcutKey::Escape));
        h.advance(Duration::from_millis(200));
    }
    h.resize_window(Extent::new(640, 480));
    h.advance(Duration::from_millis(300));
    dump(&h, "narrow", &mut out);
    let path = std::env::var_os("TEMOR_SIZE_AUDIT_OUT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| scratch_path("size-audit").join("sizes.jsonl"));
    std::fs::write(&path, out).expect("the dump");
    eprintln!("size audit written to {}", path.display());
}
