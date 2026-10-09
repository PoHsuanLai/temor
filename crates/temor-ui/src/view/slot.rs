//! The window's state that is not a signal: the session, its events, the texture, and the
//! layout that starts the session and keeps its grid fitted to the pane.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use ds_blitz::{PixelFormat, Pixels, TextureHandle};
use temor_render::{PixelSize, Scale};
use tokio::sync::mpsc::UnboundedReceiver;

use super::launch::{Launch, open_session};
use super::probe::Probe;
use crate::pane::PaneEvent;
use crate::session::{Notice, Session};

/// What the window keeps between renders that is not a signal.
#[derive(Default)]
pub struct Slot {
    pub session: Option<Session>,
    pub events: Option<UnboundedReceiver<PaneEvent>>,
    pub texture: Option<TextureHandle>,
    /// Whether the window's GPU device exists: pixels can be shown only then.
    pub device: Device,
    /// Where the screen's text is published for tests (see [`Launch::probe`]).
    pub probe: Option<Probe>,
}

impl std::fmt::Debug for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Slot")
            .field("session", &self.session)
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

/// Repaints the pane's texture (when the device exists) and publishes the screen to the probe.
pub fn repaint(slot: &RefCell<Slot>) {
    let mut slot = slot.borrow_mut();
    let Slot {
        session,
        texture,
        device,
        probe,
        ..
    } = &mut *slot;
    let Some(session) = session.as_mut() else {
        return;
    };
    if *device == Device::Ready
        && let Some(texture) = texture.as_ref()
    {
        let _ = session.paint(|bytes, size| {
            if let Ok(pixels) = Pixels::new(
                PixelFormat::Rgba8Premultiplied,
                size.width.0,
                size.height.0,
                bytes,
            ) {
                let _ = texture.update(&pixels);
            }
        });
    }
    if let Some(probe) = probe {
        probe.publish(session.screen_text());
    }
}

/// The layout step, run whenever the pane's size, scale or device changes. The first layout
/// starts the session and its event loop; later ones refit its grid. Failures come back as text
/// for the window to show.
pub fn layout(
    slot: &Rc<RefCell<Slot>>,
    ui: Signal<u32>,
    launch: &Launch,
    texture: &TextureHandle,
    pixels: PixelSize,
    scale: Scale,
    device: Device,
) -> Option<String> {
    slot.borrow_mut().device = device;
    if slot.borrow().session.is_none() {
        let (session, events) = match open_session(launch, pixels, scale) {
            Ok(pair) => pair,
            Err(err) => return Some(err.to_string()),
        };
        {
            let mut s = slot.borrow_mut();
            s.session = Some(session);
            s.texture = Some(texture.clone());
            s.events = Some(events);
            s.probe = launch.probe.clone();
        }
        spawn_event_loop(Rc::clone(slot), ui);
    }
    let failure = {
        let mut s = slot.borrow_mut();
        s.session
            .as_mut()
            .and_then(|session| session.layout(pixels, scale).err())
            .map(|err| err.to_string())
    };
    repaint(slot);
    failure
}

/// Hears the session's events for as long as the window lives: repaints on output, and marks the
/// window when the program ends.
fn spawn_event_loop(slot: Rc<RefCell<Slot>>, mut ui: Signal<u32>) {
    let Some(mut events) = slot.borrow_mut().events.take() else {
        return;
    };
    spawn(async move {
        while let Some(event) = events.recv().await {
            let notice = {
                let mut s = slot.borrow_mut();
                match s.session.as_mut() {
                    Some(session) => session.absorb(event),
                    None => break,
                }
            };
            match notice {
                Notice::Repaint => repaint(&slot),
                Notice::Ended(_) => {
                    repaint(&slot);
                    ui += 1;
                }
                Notice::Title(_) | Notice::Bell | Notice::Quiet => {}
            }
        }
    });
}
