use std::sync::Arc;

use super::atlas::{Atlases, Overflow};
use super::face::{ColorFormat, FaceRole};
use super::{FaceStyle, GlyphCache, GlyphKind, Slant, Weight};
use crate::metrics::Scale;
use crate::testing::fonts;

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
    assert!(!bold.synth.embolden, "a real bold cut");
    assert!(italic.synth.skew, "Noto Sans Mono has no italic");
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
    assert_eq!(a[0].key.kind(), GlyphKind::Mask);
    assert!(Arc::ptr_eq(&a, &cache.run("a", FaceStyle::REGULAR, 1)));
    let smile = cache.run("😀", FaceStyle::REGULAR, 2);
    assert_eq!(smile.len(), 1);
    assert_eq!(smile[0].key.kind(), GlyphKind::Color);
    assert_eq!(smile[0].w, cache.metrics().width() * 2);
    let tone = cache.run("👍\u{1f3fd}", FaceStyle::REGULAR, 2);
    assert_eq!(tone.len(), 1, "skin tone composes into one glyph");
    let space = cache.run("─", FaceStyle::REGULAR, 1);
    assert_eq!(
        (space[0].w, space[0].h),
        (cache.metrics().width(), cache.metrics().height())
    );
}

/// A glyph is named by what it is drawn from, so a name handed out stays good: a cache that let
/// its images go draws the same image from it again.
#[test]
fn a_glyphs_name_draws_the_same_image_after_the_cache_let_it_go() {
    let mut cache = GlyphCache::new(fonts(14.0), Scale::new(1.0));
    let mut kept = Vec::new();
    for text in ["a", "😀", "─", "日"] {
        for g in cache.run(text, FaceStyle::REGULAR, 2).iter() {
            let image = cache.image(g.key).cloned();
            assert!(image.is_some(), "{text}: a run's glyphs are kept");
            kept.push((text, g.key, image));
        }
    }
    // Away and back: everything kept at the first scale is let go.
    cache.set_scale(Scale::new(2.0));
    cache.set_scale(Scale::new(1.0));
    for (text, key, image) in kept {
        assert!(cache.image(key).is_none(), "{text}: let go");
        assert_eq!(cache.image_of(key).cloned(), image, "{text}");
    }
}

/// The atlases take the glyphs asked for until they are full, say so, and take them again once
/// emptied; a glyph already in is not put in twice.
#[test]
fn the_atlases_take_glyphs_until_full_and_again_once_emptied() {
    let mut cache = GlyphCache::new(fonts(120.0), Scale::new(2.0));
    let mut atlases = Atlases::with_caps(256, 256);
    let keys: Vec<_> = ('!'..='~')
        .flat_map(|c| cache.run(&c.to_string(), FaceStyle::REGULAR, 1).to_vec())
        .map(|g| g.key)
        .collect();
    let few = &keys[..4];
    assert_eq!(atlases.resolve(few.iter().copied(), &mut cache), Ok(()));
    let slots: Vec<_> = few.iter().map(|&k| atlases.slot(k)).collect();
    assert!(slots.iter().all(Option::is_some));
    assert_eq!(atlases.resolve(few.iter().copied(), &mut cache), Ok(()));
    let again: Vec<_> = few.iter().map(|&k| atlases.slot(k)).collect();
    assert_eq!(slots, again, "a glyph already in stays where it is");
    assert_eq!(
        atlases.resolve(keys.iter().copied(), &mut cache),
        Err(Overflow),
        "every printable glyph at 240 px does not fit 256 texel rows"
    );
    atlases.clear();
    assert_eq!(atlases.clears(), 1);
    assert!(few.iter().all(|&k| atlases.slot(k).is_none()));
    assert_eq!(atlases.resolve(few.iter().copied(), &mut cache), Ok(()));
}

/// The line height scales the cell's height and leaves its width alone, at every scale, and the
/// baseline, underline and strike stay inside the cell whatever the factor.
#[test]
fn the_line_height_opens_the_cell_at_any_scale_and_keeps_its_marks_inside() {
    use super::{FontConfig, FontFamily, FontSet};
    use crate::metrics::{FontSize, LineHeight};
    use crate::testing::bundled;

    let cell = |line: f32, scale: f32| {
        let set = FontSet::load(
            &FontConfig {
                family: FontFamily::Named("Noto Sans Mono".into()),
                size: FontSize::new(14.0),
                line_height: LineHeight::new(line),
            },
            bundled(),
        )
        .expect("bundled fonts load");
        GlyphCache::new(set, Scale::new(scale)).metrics()
    };
    for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
        let plain = cell(1.0, scale);
        let mut last = 0;
        for line in [0.8, 1.0, 1.2, 1.5, 2.0] {
            let m = cell(line, scale);
            assert_eq!(m.cell.width, plain.cell.width, "width at {line} x {scale}");
            let want = (plain.cell.height.0 as f32 * line).round();
            assert!(
                (m.cell.height.0 as f32 - want).abs() <= 1.0,
                "height {} for {want} at {line} x {scale}",
                m.cell.height.0
            );
            assert!(m.cell.height.0 >= last, "taller as the factor grows");
            last = m.cell.height.0;
            assert!(m.baseline.0 < m.cell.height.0, "baseline inside");
            assert!(
                m.underline.0 + m.thickness.0 <= m.cell.height.0,
                "underline inside"
            );
            assert!(
                m.strikeout.0 + m.thickness.0 <= m.cell.height.0,
                "strikeout inside"
            );
        }
    }
}

#[test]
fn symbols_no_script_owns_come_from_a_text_face_where_the_system_has_one() {
    let config = super::FontConfig::default();
    let mut set = super::FontSet::load(&config, super::FontSource::System).expect("system fonts");
    for text in ["⏵", "⎿", "⧉", "⏺"] {
        let id = set.face_for(text, FaceStyle::REGULAR);
        let face = set.face(id);
        let c = text.chars().next().unwrap();
        if !face.maps(c) {
            // A system without a symbol font (a bare container) has nothing to find; a Mac has.
            #[cfg(target_os = "macos")]
            panic!("{text} has no face on a Mac");
            #[cfg(not(target_os = "macos"))]
            continue;
        }
        if face.color == ColorFormat::None {
            continue;
        }
        // A colour face only where none of the text faces tried has the character (a runner
        // with no symbol font but an emoji one).
        let wanted = super::fallback::presentation(text);
        let text_faces: Vec<_> = set
            .candidates(text, wanted, FaceStyle::REGULAR)
            .into_iter()
            .filter(|&id| set.face(id).color == ColorFormat::None && set.face(id).maps(c))
            .collect();
        assert!(
            text_faces.is_empty(),
            "{text} is drawn in colour though a text face has it"
        );
    }
}
