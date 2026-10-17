//! A lens's picture with room made for it, in the window on the GPU harness: the demo lens
//! (`temor-plugins/examples/lens_demo.rs`) draws an `ls` block, the grid is shown in slices
//! around the room, and the picture sits in it. Ignored in the gate: it needs a GPU and the demo
//! lens built into the same target folder (`cargo build -p temor-plugins --example lens_demo`).
//! With `TEMOR_SHOT=<file.png>` it saves the frame for a person to look at.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_harness::{Backdrop, Backend, Driver, Harness, Input, WindowHosting};
use temor_ui::view::probe::Probe;

use super::support::tools_present;
use super::window::{harness_building, wait_until};

/// The demo lens built beside these tests, if it was.
fn demo_lens() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let program = exe.parent()?.parent()?.join("examples").join("lens_demo");
    program.is_file().then_some(program)
}

/// Rectangles `(left, top, width, height)` of the elements matching `selector`.
fn rects(h: &Harness, selector: &str) -> Vec<(f64, f64, f64, f64)> {
    use ds_harness::DocQuery;
    h.with_doc(|doc| {
        doc.query_selector_all(selector)
            .map(|found| found.into_iter().collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|id| doc.get_client_bounding_rect(id))
            .map(|r| (r.x, r.y, r.width, r.height))
            .collect()
    })
}

fn run_line(h: &mut Harness, probe: &Probe, line: &str, last: &str) {
    for c in line.chars() {
        let key = match c {
            ' ' => ShortcutKey::Space,
            c => ShortcutKey::Char(c),
        };
        h.send(Input::key(key));
    }
    h.send(Input::key(ShortcutKey::Enter));
    wait_until(h, probe, "the command's output", |s| {
        s.lines().any(|l| l.trim_end() == last)
    });
    h.advance(Duration::from_millis(300));
}

fn advance_until(h: &mut Harness, what: &str, ok: impl Fn(&Harness) -> bool) {
    let start = Instant::now();
    while !ok(h) {
        h.advance(Duration::from_millis(50));
        assert!(
            start.elapsed() < Duration::from_secs(30),
            "timed out waiting for {what}:\n{}",
            h.html()
        );
    }
}

#[test]
#[ignore = "needs a GPU and the demo lens built beside the tests"]
fn the_demo_lens_draws_under_its_block_in_room_between_the_rows() {
    if !tools_present() {
        return;
    }
    let lens = demo_lens().expect("build the demo lens first");
    let probe = Probe::new();
    let mut h = harness_building(
        "lens-room",
        &probe,
        None,
        |ambient| {
            let plugins = ambient.home.join(".local/share/temor/plugins");
            let demo = ambient.home.join("demo");
            std::fs::create_dir_all(&plugins).expect("plugins folder");
            std::fs::create_dir_all(&demo).expect("demo folder");
            let manifest = include_str!("../../../temor-plugins/examples/lens-demo.toml")
                .lines()
                .map(|line| match line.starts_with("path = ") {
                    true => format!("path = {:?}", lens.display().to_string()),
                    false => line.to_owned(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(plugins.join("lens-demo.toml"), manifest).expect("manifest");
            for name in [
                "a",
                "notes.txt",
                "Cargo.toml",
                "src",
                "a-much-longer-name.rs",
                "b",
            ] {
                std::fs::write(demo.join(name), "").expect("a file");
            }
        },
        WindowHosting::Absent,
        |config| config.with_backend(Backend::Hybrid),
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    h.send(Input::click(Point {
        x: Px(500.0),
        y: Px(200.0),
    }));
    h.advance(Duration::from_millis(100));
    run_line(&mut h, &probe, "ls -1 demo", "src");
    run_line(&mut h, &probe, "echo after", "after");
    // The `ls` block's card: the first on screen (its meta, or its tools in its place).
    let mut cards = rects(&h, ".temor-card-meta");
    cards.extend(rects(&h, ".temor-block-tools"));
    cards.sort_by(|a, b| a.1.total_cmp(&b.1));
    let (_, top, _, height) = *cards.first().expect("the ls card");
    let on_ls = Point {
        x: Px(300.0),
        y: Px((top + height / 2.0) as f32),
    };
    h.send(Input::button_down(on_ls, PointerButton::Secondary));
    h.send(Input::button_up(on_ls, PointerButton::Secondary));
    h.send(Input::press(on_ls, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    assert!(h.html().contains("Show with Line bars"), "{}", h.html());
    let items = rects(&h, ".ds-menu-item");
    let (x, y, w, ih) = *items.get(6).expect("the lens's item");
    h.send(Input::click(Point {
        x: Px((x + w / 2.0) as f32),
        y: Px((y + ih / 2.0) as f32),
    }));
    advance_until(&mut h, "the picture and its room", |h| {
        !rects(h, ".temor-lens").is_empty() && !rects(h, ".temor-row-gap").is_empty()
    });
    h.advance(Duration::from_millis(300));
    let (_, gap_top, _, gap_height) = rects(&h, ".temor-row-gap")[0];
    let (_, lens_top, _, _) = rects(&h, ".temor-lens")[0];
    assert!(
        (lens_top - gap_top).abs() < 0.5,
        "the picture sits in the room: {lens_top} vs {gap_top}"
    );
    assert!(gap_height > 40.0, "about six rows of room: {gap_height}");
    assert!(
        rects(&h, ".temor-grid-slice").len() >= 2,
        "the grid in slices"
    );
    let picture = h.render_over(Backdrop::Scheme).expect("a frame");
    if let Some(path) = std::env::var_os("TEMOR_SHOT") {
        let (width, height) = (picture.width(), picture.height());
        let file = std::fs::File::create(PathBuf::from(path)).expect("the shot file");
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer
            .write_image_data(picture.as_raw())
            .expect("png pixels");
    }
}
