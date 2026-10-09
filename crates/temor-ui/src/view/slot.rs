//! The window's state that is not a signal: the session, its events, the texture, and the
//! layout that starts the session and keeps its grid fitted to the pane.

use std::cell::RefCell;
use std::rc::Rc;

use std::time::Duration;

use dioxus::prelude::*;
use ds::base::time::clock::sleep;
use ds_blitz::{PixelFormat, Pixels, TextureHandle};
use temor_render::{PixelSize, Scale};
use tokio::sync::mpsc::UnboundedReceiver;

use super::launch::{Launch, open_session};
use super::probe::Probe;
use crate::pane::{Hold, PaneEvent};
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

/// How long the window keeps showing the last frame while the program holds the screen. The
/// program's own timeout is not trusted: the window's timer always releases the hold.
pub const HOLD_TIMEOUT: Duration = Duration::from_millis(150);

/// Hears the session's events for as long as the window lives. Pending events are drained
/// together and the window repaints once for them.
fn spawn_event_loop(slot: Rc<RefCell<Slot>>, mut ui: Signal<u32>) {
    let Some(mut events) = slot.borrow_mut().events.take() else {
        return;
    };
    spawn(async move {
        while let Some(first) = events.recv().await {
            let mut batch = vec![first];
            while let Ok(more) = events.try_recv() {
                batch.push(more);
            }
            let mut paint = false;
            let mut ended = false;
            let mut held_since: Option<u64> = None;
            {
                let mut s = slot.borrow_mut();
                let Some(session) = s.session.as_mut() else {
                    break;
                };
                for event in batch {
                    match session.absorb(event) {
                        Notice::Repaint => paint = true,
                        Notice::Ended(_) => {
                            ended = true;
                            paint = true;
                        }
                        Notice::Hold(Hold::Held) => {
                            // Each hold has its own generation: a stale timer cannot end a newer one.
                            held_since = Some(session.start_hold());
                        }
                        Notice::Hold(Hold::Released) => {
                            session.release_hold();
                            paint = true;
                        }
                        Notice::Title(_) | Notice::Bell | Notice::Quiet => {}
                    }
                }
            }
            if let Some(generation) = held_since {
                release_hold_later(Rc::clone(&slot), generation);
            }
            if paint {
                repaint(&slot);
            }
            if ended {
                ui += 1;
            }
        }
    });
}

/// Releases the hold with this generation once [`HOLD_TIMEOUT`] has passed, then draws the latest
/// frame. A newer hold has a newer generation and is left alone.
fn release_hold_later(slot: Rc<RefCell<Slot>>, generation: u64) {
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
