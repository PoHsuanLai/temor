//! One pane's state that is not a signal: its session, the texture it paints into, and the
//! layout that keeps its grid fitted to the room the window gives it. The window keeps one slot
//! per pane (see `hub`); only the slots of the tab in front are shown, laid out and painted.

use std::cell::RefCell;
use std::time::Duration;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use ds_blitz::{PixelFormat, Pixels, TextureHandle};
use temor_panes::render::{Palette, PixelSize, Scale};

use super::block_list::tiles::Tiles;
use super::probe::Probe;
use crate::session::Session;

/// What the window keeps for one pane between renders.
#[derive(Default)]
pub struct Slot {
    pub session: Option<Session>,
    pub texture: Option<TextureHandle>,
    /// Whether the window's GPU device exists: pixels can be shown only then.
    pub device: Device,
    /// Whether the pane is on screen. A pane in a tab that is not in front is neither laid out
    /// nor painted; it paints when its tab is shown again.
    pub display: Display,
    /// Where the screen's text is published for tests (see [`Launch::probe`](super::Launch)).
    /// Only the pane that has the keyboard publishes.
    pub probe: Option<Probe>,
    /// The grid's colours: the session opens with them, and a running one is re-coloured.
    pub palette: Palette,
    /// The block list's drawn rows, while the pane is drawn as one.
    pub tiles: Tiles,
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot")
            .field("session", &self.session)
            .field("display", &self.display)
            .finish_non_exhaustive()
    }
}

/// Whether the window has a GPU device yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Device {
    #[default]
    NotYet,
    Ready,
}

/// Whether a pane's view is mounted in the window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Display {
    #[default]
    Away,
    Shown,
}

/// Repaints the pane's texture (when it is shown and the device exists) and publishes the screen
/// to the probe. Whether the rows moved around the room under blocks: the window then draws its
/// slices of the texture again.
pub fn repaint(slot: &RefCell<Slot>) -> bool {
    let mut slot = slot.borrow_mut();
    let Slot {
        session,
        texture,
        device,
        display,
        probe,
        tiles,
        ..
    } = &mut *slot;
    let Some(session) = session.as_mut() else {
        return false;
    };
    let mut relaid = false;
    if *device == Device::Ready
        && *display == Display::Shown
        && let Some(texture) = texture.as_ref()
    {
        let painted = session.paint(|bytes, size| {
            if let Ok(pixels) = Pixels::new(
                PixelFormat::Rgba8Premultiplied,
                size.width.0,
                size.height.0,
                bytes,
            ) {
                let _ = texture.update(&pixels);
            }
        });
        relaid = painted.unwrap_or(false);
        tiles.follow_selection(session);
    }
    if let Some(probe) = probe {
        probe.publish(session.screen_text());
    }
    relaid
}

impl Slot {
    /// Makes `palette` the grid's colours from now on, and re-colours the session if it is open.
    pub fn set_palette(&mut self, palette: Palette) -> Option<String> {
        self.palette = palette;
        self.session
            .as_mut()
            .and_then(|session| session.set_palette(palette).err())
            .map(|err| err.to_string())
    }
}

/// The layout step, run whenever a shown pane's size, scale or device changes: its grid is
/// refitted and the pane repainted. A failure comes back as text for the window to show.
pub fn layout(
    slot: &RefCell<Slot>,
    pixels: PixelSize,
    scale: Scale,
    device: Device,
) -> Option<String> {
    let failure = {
        let mut s = slot.borrow_mut();
        s.device = device;
        s.session
            .as_mut()
            .and_then(|session| session.layout(pixels, scale).err())
            .map(|err| err.to_string())
    };
    repaint(slot);
    failure
}

/// How long the window keeps showing the last frame while the program holds the screen. The
/// program's own timeout is not trusted: the window's timer always releases the hold.
pub const HOLD_TIMEOUT: Duration = Duration::from_millis(150);

/// Releases the hold with this generation once [`HOLD_TIMEOUT`] has passed, then draws the latest
/// frame. A newer hold has a newer generation and is left alone.
pub fn release_hold_later(slot: std::rc::Rc<RefCell<Slot>>, generation: u64) {
    spawn(async move {
        sleep(HOLD_TIMEOUT).await;
        let ended = slot
            .borrow_mut()
            .session
            .as_mut()
            .is_some_and(|session| session.end_hold(generation));
        if ended {
            repaint(&slot);
        }
    });
}
