//! Pop Out (PLAN §7, v14 B2, ⌥⌘P): the row's page, a file, leaves its stage for a small window
//! of its own, as Arc's Little Arc does. The window is the viewer alone, in the window's look,
//! over the same edge as the stage's viewers; its Close closes the window. Back on the row brings
//! the page back to the stage.
//!
//! The call goes through [`PopOutWindows`] when the window was given one, which is how a test sees
//! what would open: quire's harness has no event loop to open a window on.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyview_pane::{FilePath, PaneEdge, PaneRequest, ViewerPane};
use dioxus::prelude::*;
use ds::base::spawner::Spawner;
use ds::prelude::*;
use ds_blitz::{TokioSpawner, WindowSize, WindowSpec, use_window_handle};

use super::launch::Launch;
use super::theme::use_window_look;

/// The size a popped-out page opens at, in logical pixels: small, beside the main window.
const SIZE: (u32, u32) = (560, 420);

/// A page popped out: its file and the edge the viewer runs over.
#[derive(Debug, Clone, PartialEq)]
pub struct PoppedOut {
    pub file: PathBuf,
    pub edge: PaneEdge,
}

/// Whatever opens a popped-out page's window: quire's event loop in the launched window, a
/// recorder in a test.
pub trait OpenPopOut: Send + Sync + 'static {
    fn open(&self, file: &Path);
}

/// Where Pop Out goes, as a root context. Without one it is quire's `open_window_with`.
#[derive(Clone)]
pub struct PopOutWindows(pub Arc<dyn OpenPopOut>);

impl std::fmt::Debug for PopOutWindows {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PopOutWindows")
    }
}

/// Open `popped` in a window of its own.
pub fn open(popped: PoppedOut) {
    if let Some(windows) = try_consume_context::<PopOutWindows>() {
        windows.0.open(&popped.file);
        return;
    }
    let title = popped
        .file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let spec = WindowSpec::new(title, WindowSize::new(SIZE.0, SIZE.1));
    if let Err(why) = ds_blitz::open_window_with(spec, pop_out_window, popped) {
        // The page has left the stage; Back brings it back.
        eprintln!("Could not pop the page out: {why}");
    }
}

fn pop_out_window(popped: PoppedOut) -> Element {
    rsx! { PopOutRoot { popped } }
}

#[component]
#[allow(non_snake_case)]
fn PopOutRoot(popped: PoppedOut) -> Element {
    let launch = use_hook(consume_context::<Launch>);
    let spawner = use_hook(|| Arc::new(TokioSpawner::current()) as Arc<dyn Spawner>);
    let env = use_window_look(&launch.appearance, spawner);
    let window = use_window_handle();
    let focused = use_signal(|| true);
    let now = env();
    let file = FilePath::new(&popped.file).ok();
    rsx! {
        Ds {
            appearance: now.settings.appearance.appearance(),
            system: now.system,
            tint_alpha: Some(now.tint_alpha()),
            stack: Some(now.material_stack()),
            typeface: Some(now.settings.appearance.typeface()),
            material: Material::Window,
            extent: RootExtent::Window,
            if let Some(file) = file {
                ViewerPane {
                    file,
                    edge: popped.edge.clone(),
                    focused,
                    on_request: move |request: PaneRequest| {
                        if let (PaneRequest::ClosePane, Some(window)) = (request, window.as_ref()) {
                            window.close();
                        }
                    },
                }
            }
        }
    }
}
