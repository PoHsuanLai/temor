//! Where a block ran, in the window (PLAN P16): the list's section line over a block names the
//! folder its prompt was in, and the composer names the folder the next command runs in.
//! The DOM is read back; no pixels, no OS input, no real HOME.

use temor_ui::view::probe::Probe;

use super::drive::{started, type_line, wait_html, wait_until};
use super::support::window_tools_present;

/// The text of the first element whose opening tag carries `class`, if any.
fn text_of<'a>(html: &'a str, class: &str) -> Option<&'a str> {
    let after = html.split(&format!("class=\"{class}\"")).nth(1)?;
    let (_, rest) = after.split_once('>')?;
    rest.split('<').next()
}

#[test]
fn a_blocks_section_and_the_composer_name_the_folder() {
    if !window_tools_present() {
        return;
    }
    let probe = Probe::new();
    let mut h = started("facts-folder", &probe);
    type_line(&mut h, "echo here");
    wait_until(&mut h, &probe, "the output", |s| {
        s.lines().any(|l| l.trim_end() == "here")
    });
    wait_html(&mut h, "the section line", |html| {
        text_of(html, "temor-bseg temor-blk-section") == Some("~")
    });
    wait_html(&mut h, "the composer's folder", |html| {
        text_of(html, "temor-blk-where") == Some("~")
    });
}
