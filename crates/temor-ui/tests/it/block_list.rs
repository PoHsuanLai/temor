//! The pane as a list of blocks in a real window: a click on a card picks its block, and a drag
//! across cards' rows selects their text and lets go of the pick. The list's cards are found by
//! their boxes, as a person would point at them.

use std::time::Duration;

use ds::base::press::PointerButton;
use ds::base::vocab::ShortcutKey;
use ds::prelude::{Point, Px};
use ds_blitz::Extent;
use ds_harness::{Clock, Input, WindowHosting};
use temor_ui::view::probe::Probe;

use super::drive::{Rect, rects, started, type_line, wait_html, wait_until};
use super::window::harness_building;

/// The heads of the cards, oldest first.
fn heads(h: &ds_harness::Harness) -> Vec<Rect> {
    rects(h, ".temor-blk-head")
}

/// How many card heads say they are picked.
fn picked(html: &str) -> usize {
    html.matches("data-picked=\"true\"").count()
}

/// A point `dx` into a box from its left, halfway down.
fn left_of(r: Rect, dx: f64) -> Point {
    Point {
        x: Px((r.x + dx) as f32),
        y: Px((r.y + r.h / 2.0) as f32),
    }
}

#[test]
fn a_click_on_a_card_picks_it_and_a_drag_across_cards_selects_their_text() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("block-list-pointer", &probe);
    for word in ["alpha", "beta", "gamma"] {
        type_line(&mut h, &format!("echo {word}"));
        wait_until(&mut h, &probe, "the output", |s| {
            s.lines().any(|l| l.trim_end() == word)
        });
    }
    wait_html(&mut h, "three cards", |html| {
        html.matches("temor-blk-head").count() >= 3
    });
    let found = heads(&h);
    let beta = found[found.len() - 2];
    h.send(Input::click(left_of(beta, 120.0)));
    h.advance(Duration::from_millis(300));
    wait_html(&mut h, "the second card picked", |html| picked(html) == 1);
    let html = h.html();
    let at = html.find("data-picked=\"true\"").expect("a picked head");
    assert!(
        html[at..].find("echo beta") < html[at..].find("echo gamma"),
        "the picked head is beta's"
    );
    // From the start of beta's output to the end of gamma's: a selection, so no pick.
    let rows = rects(&h, ".temor-blk-rows");
    let (from, to) = (rows[rows.len() - 2], rows[rows.len() - 1]);
    h.send(Input::drag(left_of(from, 10.0), left_of(to, 60.0), 12));
    h.advance(Duration::from_millis(300));
    wait_html(&mut h, "the pick let go", |html| picked(html) == 0);
}

/// Turns the wheel `clicks` over `at` (winit's sign: positive is towards older output), as the
/// window's scroll engine takes it, and lets the list settle.
fn wheel(h: &mut ds_harness::Harness, at: Point, clicks: f64) {
    h.send(Input::detents(at, 0.0, clicks));
    h.advance(Duration::from_millis(400));
}

/// The top of the newest card's foot, in the window.
fn newest_foot(h: &ds_harness::Harness) -> f64 {
    rects(h, ".temor-blk-foot").last().map_or(0.0, |r| r.y)
}

#[test]
fn the_wheel_scrolls_the_list_back_and_a_new_command_brings_it_to_its_bottom() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    // The harness's own clock: the scroll engine eases once a frame, as in a window, not once a
    // round of a frame's renders.
    let mut h = harness_building(
        "block-list-wheel",
        &probe,
        None,
        |_| {},
        WindowHosting::Absent,
        |config| config.with_clock(Clock::Virtual),
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    type_line(&mut h, "seq 1 300");
    wait_until(&mut h, &probe, "the output", |s| {
        s.lines().any(|l| l.trim_end() == "300")
    });
    wait_html(&mut h, "its card", |html| html.contains("temor-blk-foot"));
    h.advance(Duration::from_millis(300));
    let list = rects(&h, ".temor-blist")[0];
    let middle = Point {
        x: Px((list.x + list.w / 2.0) as f32),
        y: Px((list.y + list.h / 2.0) as f32),
    };
    // At the bottom the long card's foot is in view, above the prompt.
    let bottom = newest_foot(&h);
    assert!(
        (list.y..list.y + list.h).contains(&bottom),
        "the foot is in view: {bottom} in {list:?}"
    );
    wheel(&mut h, middle, 5.0);
    let back = newest_foot(&h);
    assert!(back > bottom + 100.0, "scrolled back: {bottom} -> {back}");
    type_line(&mut h, "echo after");
    wait_html(&mut h, "the new card", |html| html.contains("echo after"));
    h.advance(Duration::from_millis(300));
    // A new command is a new newest block: the list goes back to its bottom.
    let last = heads(&h).last().copied().expect("a head");
    assert!(
        last.y + last.h <= list.y + list.h,
        "the newest card is in view: {last:?} in {list:?}"
    );
}

#[test]
fn a_narrower_window_reflows_a_cards_rows_and_the_card_grows_to_hold_them() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("block-list-reflow", &probe);
    // One line of output 150 characters long: one row wide, several narrow.
    type_line(&mut h, "printf '%0150d\\n' 7");
    wait_until(&mut h, &probe, "the output", |s| s.contains("00007"));
    wait_html(&mut h, "its card", |html| html.contains("temor-blk-rows"));
    h.advance(Duration::from_millis(300));
    let wide = rects(&h, ".temor-blk-rows")[0].h;
    h.resize_window(Extent::new(520, 640));
    h.advance(Duration::from_millis(600));
    wait_html(&mut h, "the card again", |html| {
        html.contains("temor-blk-rows")
    });
    let narrow = rects(&h, ".temor-blk-rows")[0].h;
    assert!(
        narrow > wide + 10.0,
        "the rows wrapped and the card grew: {wide} -> {narrow}"
    );
    // The command's line stays the card's head, not one of its rows.
    assert!(h.html().contains("printf"), "the head:\n{}", h.html());
}

#[test]
#[ignore = "needs the demo lens built beside the tests"]
fn a_lens_picture_shows_in_its_card_between_the_rows_and_the_foot() {
    if !super::support::window_tools_present() {
        return;
    }
    let lens = temor_testkit::example_program("lens_demo");
    let probe = Probe::new();
    let mut h = harness_building(
        "block-list-lens",
        &probe,
        None,
        |ambient| {
            let plugins = ambient.home.join(".local/share/temor/plugins");
            let demo = ambient.home.join("demo");
            std::fs::create_dir_all(&plugins).expect("plugins folder");
            std::fs::create_dir_all(&demo).expect("demo folder");
            let manifest = temor_testkit::with_program(
                include_str!("../../../temor-plugins/examples/lens-demo.toml"),
                &lens,
            );
            std::fs::write(plugins.join("lens-demo.toml"), manifest).expect("manifest");
            for name in ["a", "notes.txt", "Cargo.toml", "src", "b"] {
                std::fs::write(demo.join(name), "").expect("a file");
            }
        },
        WindowHosting::Absent,
        |config| config,
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    type_line(&mut h, "ls -1 demo");
    wait_until(&mut h, &probe, "the listing", |s| {
        s.lines().any(|l| l.trim_end() == "src")
    });
    type_line(&mut h, "echo after");
    wait_until(&mut h, &probe, "the second block", |s| {
        s.lines().any(|l| l.trim_end() == "after")
    });
    wait_html(&mut h, "two cards", |html| {
        html.matches("temor-blk-foot").count() >= 2
    });
    h.advance(Duration::from_millis(300));
    let ls = heads(&h)[0];
    let on_ls = left_of(ls, 160.0);
    h.send(Input::button_down(on_ls, PointerButton::Secondary));
    h.send(Input::button_up(on_ls, PointerButton::Secondary));
    h.advance(Duration::from_millis(300));
    assert!(h.html().contains("Show with Line bars"), "{}", h.html());
    let items = rects(&h, ".ds-menu-item");
    let item = *items.get(7).expect("the lens's item");
    h.send(Input::click(item.centre()));
    wait_html(&mut h, "the picture", |html| {
        html.contains("temor-blk-picture")
    });
    h.advance(Duration::from_millis(300));
    let picture = rects(&h, ".temor-blk-picture")[0];
    let rows = rects(&h, ".temor-blk-rows")[0];
    let foot = rects(&h, ".temor-blk-foot")[0];
    assert!(
        (picture.y - (rows.y + rows.h)).abs() < 0.5,
        "under the rows: {picture:?} {rows:?}"
    );
    assert!(
        (foot.y - (picture.y + picture.h)).abs() < 0.5,
        "above the foot: {picture:?} {foot:?}"
    );
    let img = rects(&h, ".temor-blk-img")[0];
    assert!(img.h > 20.0 && img.w <= picture.w, "{img:?} in {picture:?}");
    // Folded, the card is its head and foot: the picture goes with the rows.
    h.send(Input::click(rects(&h, ".temor-blk-fold")[0].centre()));
    h.advance(Duration::from_millis(300));
    wait_html(&mut h, "the picture folded away", |html| {
        !html.contains("temor-blk-picture")
    });
}

/// The composer field's value, as the page writes it.
fn composer_value(html: &str) -> Option<String> {
    let card = &html[html.find("temor-blk-composer")?..];
    let at = [card.find("<input"), card.find("<textarea")]
        .into_iter()
        .flatten()
        .min()?;
    let input = &card[at..];
    let value = &input[input.find("value=\"")? + 7..];
    Some(value[..value.find('"')?].to_owned())
}

#[test]
fn the_composer_takes_the_command_sends_it_on_enter_and_steps_back_through_the_history() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("block-list-composer", &probe);
    wait_html(&mut h, "the composer", |html| {
        html.contains("temor-blk-composer")
    });
    super::drive::type_text(&mut h, "echo one");
    h.advance(Duration::from_millis(200));
    assert_eq!(
        composer_value(&h.html()).as_deref(),
        Some("echo one"),
        "typed into the composer"
    );
    // Nothing reached the shell's own line yet: the prompt's rows hold no command.
    assert!(
        !probe.screen().contains("echo one"),
        "the shell has not seen it:\n{}",
        probe.screen()
    );
    h.send(Input::key(ShortcutKey::Enter));
    wait_until(&mut h, &probe, "its output", |s| {
        s.lines().any(|l| l.trim_end() == "one")
    });
    wait_html(&mut h, "its card, and the composer empty again", |html| {
        html.contains("echo one") && composer_value(html).as_deref() == Some("")
    });
    type_line(&mut h, "echo two");
    wait_until(&mut h, &probe, "the second output", |s| {
        s.lines().any(|l| l.trim_end() == "two")
    });
    wait_html(&mut h, "the composer back", |html| {
        composer_value(html).as_deref() == Some("")
    });
    h.send(Input::key(ShortcutKey::Up));
    h.advance(Duration::from_millis(200));
    assert_eq!(composer_value(&h.html()).as_deref(), Some("echo two"));
    h.send(Input::key(ShortcutKey::Up));
    h.advance(Duration::from_millis(200));
    assert_eq!(composer_value(&h.html()).as_deref(), Some("echo one"));
    h.send(Input::key(ShortcutKey::Down));
    h.send(Input::key(ShortcutKey::Down));
    h.advance(Duration::from_millis(200));
    assert_eq!(
        composer_value(&h.html()).as_deref(),
        Some(""),
        "back to what was typed"
    );
}

#[test]
#[ignore = "a picture for a person; with TEMOR_SHOT=<file.png> it saves the frame for a person to look at"]
fn a_picture_of_the_list_with_its_cards_and_the_composer() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = harness_building(
        "block-list-shot",
        &probe,
        None,
        |_| {},
        WindowHosting::Absent,
        |config| config,
    );
    wait_until(&mut h, &probe, "the prompt", |s| s.contains('$'));
    for (line, last) in [
        ("printf 'alpha\\nbeta\\ngamma\\n'", "gamma"),
        ("ls /nope", "directory"),
        ("seq 1 12", "12"),
    ] {
        type_line(&mut h, line);
        wait_until(&mut h, &probe, "the output", |s| s.contains(last));
    }
    wait_html(&mut h, "the composer back", |html| {
        composer_value(html).as_deref() == Some("")
    });
    super::drive::type_text(&mut h, "git status");
    h.advance(Duration::from_millis(500));
    let picture = h
        .render_over(ds_harness::Backdrop::Scheme)
        .expect("a frame");
    if let Some(path) = std::env::var_os("TEMOR_SHOT") {
        let (width, height) = (picture.width(), picture.height());
        let file = std::fs::File::create(std::path::PathBuf::from(path)).expect("the shot file");
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().expect("png header");
        writer
            .write_image_data(picture.as_raw())
            .expect("png pixels");
    }
}

/// The composer card's height, in window pixels.
fn composer_height(h: &ds_harness::Harness) -> f64 {
    rects(h, ".temor-blk-composer").first().map_or(0.0, |r| r.h)
}

#[test]
fn shift_enter_starts_a_new_line_the_card_grows_and_enter_sends_both() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("block-list-composer-lines", &probe);
    wait_html(&mut h, "the composer", |html| {
        html.contains("temor-blk-composer")
    });
    let one = composer_height(&h);
    super::drive::type_text(&mut h, "echo one");
    h.send(Input::chord(&[ShortcutKey::Shift], ShortcutKey::Enter));
    h.advance(Duration::from_millis(300));
    super::drive::type_text(&mut h, "echo two");
    h.advance(Duration::from_millis(300));
    let html = h.html();
    assert!(html.contains("<textarea"), "a field of lines:\n{html}");
    let value = composer_value(&html).unwrap_or_default();
    assert!(
        value.contains("echo one") && value.contains("echo two") && value.contains('\n')
            || value.contains("echo one&#10;echo two"),
        "both lines in the field: {value:?}"
    );
    let two = composer_height(&h);
    assert!(two > one + 10.0, "the card grew: {one} -> {two}");
    // Nothing reached the shell yet; Enter sends both lines.
    assert!(!probe.screen().contains("echo one"), "{}", probe.screen());
    h.send(Input::key(ShortcutKey::Enter));
    wait_until(&mut h, &probe, "both outputs", |s| {
        s.lines().any(|l| l.trim_end() == "one") && s.lines().any(|l| l.trim_end() == "two")
    });
    wait_html(&mut h, "a card for the two lines", |html| {
        html.contains(">echo one<")
    });
    wait_html(&mut h, "the composer one line again", |html| {
        composer_value(html).as_deref() == Some("") && !html.contains("<textarea")
    });
    assert!((composer_height(&h) - one).abs() < 1.0, "back to one line");
}

#[test]
fn the_wheel_over_a_program_that_asked_for_the_mouse_goes_to_it() {
    if !super::support::window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("block-list-wheel-program", &probe);
    // A program on the shell's screen that asks for the mouse (SGR reports) and prints the first
    // six bytes it reads.
    type_line(
        &mut h,
        "stty raw -echo; printf 'ready\\033[?1000h\\033[?1006h\\r\\n'; \
         dd bs=1 count=6 2>/dev/null | od -An -c; printf '\\033[?1000l\\033[?1006l'; stty sane",
    );
    wait_until(&mut h, &probe, "the program", |s| {
        s.lines().any(|l| l.trim_end() == "ready")
    });
    wait_html(&mut h, "its live rows", |html| {
        html.contains("data-wheel=\"capture\"")
    });
    let live = rects(&h, ".temor-blk-rows")
        .into_iter()
        .max_by(|a, b| a.y.total_cmp(&b.y))
        .expect("live rows");
    wheel(&mut h, left_of(live, 40.0), 1.0);
    // A wheel up is button 64 in an SGR report: ESC [ < 6 4 ;
    wait_until(&mut h, &probe, "the report", |s| {
        s.contains("033   [   <   6   4   ;")
    });
}
