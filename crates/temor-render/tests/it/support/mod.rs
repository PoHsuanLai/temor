//! A terminal wired to a renderer with the bundled fonts, and golden-image comparison.

mod golden;

pub use golden::assert_golden;

use std::sync::mpsc;

use temor_render::{
    CpuRenderer, FontConfig, FontFamily, FontSet, FontSize, FontSource, FrameCapture, Padding,
    Palette, PaneFocus, PaneFrame, PaneView, PixelSize, RenderStats, Renderer, Scale,
};
use temor_vt::{GridSize, Px, Terminal, TerminalConfig, VtEvent};

/// The bundled OFL fonts (tests/fonts): Noto Sans Mono, then CJK and colour emoji fallbacks.
pub fn fonts(px: f32) -> FontSet {
    let dir = format!("{}/tests/fonts", env!("CARGO_MANIFEST_DIR"));
    let files = [
        "NotoSansMono-Regular-subset.ttf",
        "NotoSansMono-Bold-subset.ttf",
        "NotoSansMonoCJK-subset.otf",
        "NotoColorEmoji-subset.ttf",
    ]
    .map(|f| std::fs::read(format!("{dir}/{f}")).unwrap_or_else(|e| panic!("{f}: {e}")));
    let config = FontConfig {
        family: FontFamily::Named("Noto Sans Mono".into()),
        size: FontSize::new(px),
    };
    FontSet::load(&config, FontSource::Bundled(files.to_vec())).expect("bundled fonts")
}

/// The bytes of `tests/fixtures/<name>`.
pub fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// One terminal pane: the terminal, its capture, its frame and a CPU renderer, sized so the
/// grid fits exactly inside `PADDING` at `scale`.
pub struct Pane {
    pub term: Terminal,
    _events: mpsc::Receiver<VtEvent>,
    capture: FrameCapture,
    pub frame: PaneFrame,
    pub renderer: CpuRenderer,
    pub size: PixelSize,
    pub scale: Scale,
    pub palette: Palette,
}

pub const PADDING: u32 = 4;

/// The output of one render, owned.
pub struct Shot {
    pub rgba: Vec<u8>,
    pub size: PixelSize,
    pub damage: Vec<temor_render::DamageRect>,
    pub stats: RenderStats,
}

impl Pane {
    pub fn new(cols: u16, rows: u16, scale: f32, font_px: f32) -> Self {
        let scale = Scale::new(scale);
        let mut renderer = CpuRenderer::new(fonts(font_px));
        let m = renderer.cell_metrics(scale);
        let pad = Padding::uniform(Px(PADDING)).left.0 * 2;
        let size = PixelSize::new(
            u32::from(cols) * m.width() + pad,
            u32::from(rows) * m.height() + pad,
        );
        let (tx, events) = mpsc::channel();
        let (reply_tx, _replies) = mpsc::channel::<Vec<u8>>();
        let mut config = TerminalConfig::new(GridSize::new(cols, rows));
        config.cell_px = m.cell;
        let mut term = Terminal::new(config, Box::new(tx), Box::new(reply_tx)).expect("terminal");
        let palette = Palette::default();
        palette.apply(&mut term).expect("palette");
        Self {
            term,
            _events: events,
            capture: FrameCapture::new().expect("capture"),
            frame: PaneFrame::new(),
            renderer,
            size,
            scale,
            palette,
        }
    }

    pub fn write(&mut self, bytes: &[u8]) {
        self.term.write(bytes);
    }

    /// Captures and renders a frame.
    pub fn render(&mut self, focus: PaneFocus) -> Shot {
        self.capture
            .begin(&mut self.term)
            .expect("begin")
            .finish(&mut self.frame)
            .expect("finish");
        let view = PaneView {
            size: self.size,
            scale: self.scale,
            padding: Padding::uniform(Px(PADDING)),
            palette: &self.palette,
            focus,
            selection: None,
        };
        let out = self
            .renderer
            .render(&mut self.frame, &view)
            .expect("render");
        Shot {
            rgba: out.pixels.to_vec(),
            size: out.size,
            damage: out.damage.to_vec(),
            stats: out.stats,
        }
    }
}
