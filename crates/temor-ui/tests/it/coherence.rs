//! The four coherence rules (quire's `CONSUMING.md` section 5) for temor's window, as quire's
//! consumer example proves them (`examples/consumer/tests/it/coherence.rs` at its pin):
//! 1. [`our_stylesheet_lints_clean`]: no literal design value in the window's own CSS.
//! 2. [`rendered_markup_uses_only_styled_classes`] and [`no_raw_form_control_or_svg_is_rendered`]:
//!    no raw markup, no class nothing styles.
//! 3. Rule 3 (a missing component is added to quire first) is a review discipline, not a check.
//! 4. Rule 4 (motion only through quire's timers) has no test yet: the window draws no motion of
//!    its own (the bell's mark is a timer of the window's own, asked of quire, see FOLLOWUPS).

use ds_lint::{LintConfig, Profile, Rule, assert_clean, markup};
use temor_ui::view::probe::Probe;
use temor_ui::view::window::WINDOW_CSS;

use super::support::tools_present;
use super::window::harness;

/// Classes quire's own components draw that no stylesheet defines: quire styles them inline (the
/// texture layer's box). Ours are never on this list; quire is asked to define them (FOLLOWUPS).
const QUIRE_INLINE_CLASSES: &[&str] = &["ds-texture-layer"];

/// What `markup` needs to know is styled: quire's own stylesheet, plus the window's.
fn window_css() -> String {
    format!("{}\n{WINDOW_CSS}", ds::stylesheet())
}

/// The window's first frame as the harness renders it.
fn first_frame() -> String {
    let probe = Probe::new();
    harness("coherence-markup", &probe).html()
}

/// Rule 1: the window's CSS under the strictest profile, the call `CONSUMING.md` section 5 shows.
#[test]
fn our_stylesheet_lints_clean() {
    assert_clean(
        WINDOW_CSS,
        &LintConfig {
            profile: Profile::Strict,
            ..LintConfig::new(&ds::kits())
        },
    );
}

/// Rule 2: every class the first frame carries is one quire's stylesheet or the window's styles.
#[test]
fn rendered_markup_uses_only_styled_classes() {
    if !tools_present() {
        return;
    }
    let html = first_frame();
    let offences = markup(&html, &window_css(), &LintConfig::new(&ds::kits()));
    let unstyled: Vec<_> = offences
        .iter()
        .filter(|offence| offence.rule == Rule::UnstyledClass)
        .filter(|offence| {
            !QUIRE_INLINE_CLASSES
                .iter()
                .any(|c| offence.text.contains(c))
        })
        .collect();
    assert!(unstyled.is_empty(), "{unstyled:#?}\nin:\n{html}");
}

/// Rule 2: no hand-written `<button>`, `<input>`, `<select>`, `<textarea>` or `<svg>`.
#[test]
fn no_raw_form_control_or_svg_is_rendered() {
    if !tools_present() {
        return;
    }
    let html = first_frame();
    let offences = markup(&html, &window_css(), &LintConfig::new(&ds::kits()));
    let raw: Vec<_> = offences
        .iter()
        .filter(|offence| offence.rule == Rule::RawMarkup)
        .collect();
    assert!(raw.is_empty(), "{raw:#?}\nin:\n{html}");
}
