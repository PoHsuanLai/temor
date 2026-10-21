//! The bundled test fonts (tests/fonts, OFL).

use crate::font::{FontConfig, FontFamily, FontSet, FontSource};
use crate::metrics::FontSize;

/// Noto Sans Mono, then the CJK and colour emoji fallbacks, as bytes.
pub fn bundled() -> FontSource {
    FontSource::Bundled(vec![
        include_bytes!("../../tests/fonts/NotoSansMono-Regular-subset.ttf").to_vec(),
        include_bytes!("../../tests/fonts/NotoSansMono-Bold-subset.ttf").to_vec(),
        include_bytes!("../../tests/fonts/NotoSansMonoCJK-subset.otf").to_vec(),
        include_bytes!("../../tests/fonts/NotoColorEmoji-subset.ttf").to_vec(),
    ])
}

/// Noto Sans Mono at `px` logical pixels, with the CJK and emoji fallbacks.
pub fn fonts(px: f32) -> FontSet {
    let config = FontConfig {
        family: FontFamily::Named("Noto Sans Mono".into()),
        size: FontSize::new(px),
        ..FontConfig::default()
    };
    FontSet::load(&config, bundled()).expect("bundled fonts load")
}
