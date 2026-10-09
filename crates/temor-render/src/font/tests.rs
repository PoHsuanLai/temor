use std::sync::Arc;

use super::face::{ColorFormat, FaceRole};
use super::testing::fonts;
use super::{FaceStyle, GlyphCache, GlyphKind, Slant, Weight};
use crate::metrics::Scale;

const BOLD: FaceStyle = FaceStyle {
    weight: Weight::Bold,
    slant: Slant::Upright,
};
const ITALIC: FaceStyle = FaceStyle {
    weight: Weight::Regular,
    slant: Slant::Italic,
};

#[test]
fn the_named_family_gives_four_cuts_with_synthetic_italic() {
    let set = fonts(14.0);
    let regular = set.face(set.primary(FaceStyle::REGULAR));
    let bold = set.face(set.primary(BOLD));
    let italic = set.face(set.primary(ITALIC));
    assert_eq!(regular.role, FaceRole::Primary);
    assert_ne!(set.primary(FaceStyle::REGULAR), set.primary(BOLD));
    assert_eq!(bold.embolden, super::face::Embolden::No, "a real bold cut");
    assert_eq!(
        italic.skew,
        super::face::Skew::Yes,
        "Noto Sans Mono has no italic"
    );
}

#[test]
fn fallbacks_cover_cjk_and_colour_emoji() {
    let mut set = fonts(14.0);
    let own = set.primary(FaceStyle::REGULAR);
    assert_eq!(set.face_for("a", FaceStyle::REGULAR), own);
    let cjk = set.face_for("日", FaceStyle::REGULAR);
    assert_ne!(cjk, own);
    assert!(set.face(cjk).maps('日'));
    let emoji = set.face_for("😀", FaceStyle::REGULAR);
    assert_eq!(set.face(emoji).color, ColorFormat::Colr);
    let family = set.face_for("👨\u{200d}👩\u{200d}👧", FaceStyle::REGULAR);
    assert_eq!(family, emoji);
}

#[test]
fn metrics_are_whole_pixels_and_scale_up() {
    let mut cache = GlyphCache::new(fonts(14.0), Scale::new(1.0));
    let one = cache.metrics();
    assert!(one.width() >= 7 && one.height() >= 14, "{one:?}");
    assert!(one.baseline.0 < one.height());
    assert!(one.underline.0 + one.thickness.0 <= one.height());
    cache.set_scale(Scale::new(2.0));
    let two = cache.metrics();
    assert!(two.width() >= one.width() * 2 - 1 && two.width() <= one.width() * 2 + 1);
}

#[test]
fn runs_are_cached_and_emoji_are_colour() {
    let mut cache = GlyphCache::new(fonts(14.0), Scale::new(1.0));
    let a = cache.run("a", FaceStyle::REGULAR, 1);
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].kind, GlyphKind::Mask);
    assert!(Arc::ptr_eq(&a, &cache.run("a", FaceStyle::REGULAR, 1)));
    let smile = cache.run("😀", FaceStyle::REGULAR, 2);
    assert_eq!(smile.len(), 1);
    assert_eq!(smile[0].kind, GlyphKind::Color);
    assert_eq!(smile[0].slot.w, cache.metrics().width() * 2);
    let tone = cache.run("👍\u{1f3fd}", FaceStyle::REGULAR, 2);
    assert_eq!(tone.len(), 1, "skin tone composes into one glyph");
    let space = cache.run("─", FaceStyle::REGULAR, 1);
    assert_eq!(
        (space[0].slot.w, space[0].slot.h),
        (cache.metrics().width(), cache.metrics().height())
    );
}

#[test]
fn a_full_atlas_clears_and_moves_the_epoch() {
    let mut cache = GlyphCache::new(fonts(120.0), Scale::new(2.0));
    let start = cache.epoch();
    let text: Vec<String> = ('!'..='~').map(String::from).collect();
    for t in text.iter().cycle().take(400) {
        cache.run(t, FaceStyle::REGULAR, 1);
        cache.run(t, BOLD, 1);
    }
    assert!(cache.epoch() > start);
    assert!(!cache.run("A", FaceStyle::REGULAR, 1).is_empty());
}
