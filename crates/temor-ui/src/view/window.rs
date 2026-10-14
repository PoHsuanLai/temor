//! The window (PLAN §2, §7): the sidebar of tabs beside the tab in front. The window owns the
//! hub (its panes and workspace), starts the loops that hear the panes and save the session, and
//! draws the look: the root, the palette, the title and the message bar. The panes are
//! `pane_view`, the tab's tree is `tree_view`, the sidebar is `tab_sidebar`.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use dioxus::prelude::*;
use ds::base::spawner::Spawner;
use ds::base::vocab::Shown;
use ds::components::app::spaces::SIDEBAR_MIN_WIDTH;
use ds::components::chrome::split_view::model::{PaneSize, PaneSpec, SplitPane};
use ds::components::chrome::split_view::view::SplitView;
use ds::components::overlays::drag_ghost::DragGhost;
use ds::host::measure::use_rect;
use ds::motion::drag::{DRAG_THRESHOLD, DragPhase, use_drag};
use ds::prelude::*;
use ds::window::host::{use_window_host, use_window_title};
use ds_blitz::{TokioSpawner, use_gpu, use_window_handle};
use ds_settings::{AppearanceFile, use_environment};
use temor_core::split::Direction;
use temor_core::vocab::ids::TabId;
use temor_core::vocab::text::Relaunch;
use temor_core::workspace::{TabsIn, WorkspaceIn};

use super::bar::BarView;
use super::dnd::{ClickCell, Dnd, Dropped, RowRects, point_of, resolve};
use super::launch::Launch;
use super::palette::PaletteFollow;
use super::quick::QuickView;
use super::radar::AgentSheets;
use super::sidebar_room::SidebarRoom;
use super::space_ui::{SpaceMenus, use_window_spaces};
use super::tab_sidebar::TabSidebar;
use super::theme::dark_by_default;
use super::title::window_title;
use super::transfer_ask::TransferAsk;
use super::tree_view::TreeView;
use super::window_style::WINDOW_CSS;
use crate::app::Desktop;
use crate::edge::{Credentials, read_theme_choice};
use crate::hub::{
    Desk, DragKey, HostOf, Hub, HubRef, KitSpaces, PendingSwitch, SidebarData, spawn_agent_loops,
    spawn_blink_loop, spawn_desktop_loop, spawn_event_loop, spawn_facts_loop, spawn_keys_loop,
    spawn_offer_loop, spawn_remote_loop, spawn_running_loop, spawn_save_loop, spawn_ssh_loop,
    spawn_viewer_loop,
};

/// The window's root component.
#[allow(non_snake_case)]
pub fn App() -> Element {
    let launch = use_hook(consume_context::<Launch>);
    // The window's GPU is found (or provided) here, once, for every pane under it.
    let _gpu = use_gpu();
    let started = use_hook(Instant::now);
    let spawner = use_hook(|| Arc::new(TokioSpawner::current()) as Arc<dyn Spawner>);
    // The person's look and the desktop's preferences, live: a change re-themes the window.
    let stored = use_environment(
        launch.appearance.store.clone(),
        launch.appearance.system.clone(),
        spawner,
    );
    // Dark unless the person's file names a theme; the file is read again when it changes.
    let choice = use_memo({
        let store = launch.appearance.store.clone();
        move || {
            let _ = stored();
            read_theme_choice(store.path::<AppearanceFile>().as_deref())
        }
    });
    let env = use_memo(move || dark_by_default(stored(), choice()));
    let hub = use_hook({
        let launch = launch.clone();
        move || {
            let mut hub = Hub::new(launch);
            hub.set_credentials(try_consume_context::<Credentials>().unwrap_or_default());
            if let Some(desktop) = try_consume_context::<Desktop>() {
                hub.set_tailnet(desktop.tailnet.clone());
                hub.set_checkpoints(desktop.checkpoints.clone());
            }
            hub.start();
            HubRef(Rc::new(RefCell::new(hub)))
        }
    });
    let ui = use_signal(|| 0u32);
    let mut notice = use_signal({
        let hub = hub.clone();
        move || hub.0.borrow_mut().take_notice()
    });
    let spaces = use_window_spaces(&hub, ui, notice);
    // The Space on screen decides the look: the frame, the sidebar's ink and the terminal's colours.
    let look = spaces.look();
    let picked_space = use_hook(PendingSwitch::default);
    use_hook({
        let (hub, picked) = (hub.clone(), picked_space.clone());
        move || hub.set_bar_spaces(Rc::new(KitSpaces::new(spaces, picked)))
    });
    let mut sidebar = use_signal(|| Shown::Visible);
    let shelf = use_signal(|| Shown::Hidden);
    let desk = Desk {
        sidebar,
        shelf,
        hub: hub.clone(),
        ui,
        notice,
        spaces,
        picked_space,
        window: HostOf(use_window_host()),
    };
    use_hook({
        let desk = desk.clone();
        move || {
            spawn_event_loop(desk.clone());
            spawn_keys_loop(desk.clone());
            spawn_agent_loops(desk.clone());
            spawn_facts_loop(desk.clone());
            spawn_ssh_loop(desk.clone());
            spawn_viewer_loop(desk.clone());
            spawn_offer_loop(desk.clone());
            spawn_running_loop(desk.clone());
            spawn_blink_loop(desk.clone());
            spawn_remote_loop(desk.clone());
            spawn_save_loop(desk.hub);
        }
    });
    // What the desktop services are given of the window; a window without them has none.
    let desktop = use_hook(|| try_consume_context::<Desktop>().unwrap_or_else(Desktop::none));
    // The window's own handle, so a second launch can raise it (none in the harness).
    let window = use_window_handle();
    use_hook({
        let (desk, desktop) = (desk.clone(), desktop.clone());
        move || {
            if let Some(link) = desktop.link {
                spawn_desktop_loop(desk, link, window);
            }
        }
    });
    let side = use_rect();
    // A window narrower than the breakpoint starts without the sidebar, and one that crosses it
    // starts over from what its width suggests. The show/hide action overrides in between.
    let shell = use_rect();
    let mut room = use_signal(|| None::<SidebarRoom>);
    use_effect(move || {
        let Some(rect) = shell.rect() else { return };
        let now = SidebarRoom::of(rect.size.width);
        if *room.peek() != Some(now) {
            room.set(Some(now));
            sidebar.set(now.shown());
        }
    });
    let dnd = Dnd {
        drag: use_drag::<DragKey>(DRAG_THRESHOLD),
        rects: use_hook(RowRects::default),
        over: use_signal(|| None),
        click: use_hook(ClickCell::default),
    };

    // A release: a tab joins the tab it is over, a tile past the sidebar's edge tears off.
    let release = {
        let (dnd, desk) = (dnd.clone(), desk.clone());
        use_callback(move |(): ()| {
            let phase = dnd.drag.phase();
            let mut over = dnd.over;
            let target = *over.peek();
            over.set(None);
            dnd.drag.up();
            if matches!(phase, DragPhase::Live { .. }) {
                dnd.click.swallow();
            }
            let edge = side.rect().map(|r| r.right());
            let pins = desk.hub.sidebar().pinned;
            let pinned = |tab: TabId| pins.iter().any(|row| row.id == tab);
            let tabs = |input: TabsIn| desk.act(WorkspaceIn::Tabs(input));
            match resolve(&phase, target, edge, pinned) {
                Dropped::Join { from, onto } => {
                    if let Some(pane) = desk.hub.focus_pane_of(onto) {
                        desk.act(WorkspaceIn::DropTab {
                            from,
                            onto: pane,
                            towards: Direction::Right,
                        });
                    }
                }
                Dropped::Place { from, onto } => tabs(TabsIn::Place {
                    tab: from,
                    at: onto,
                }),
                Dropped::Unpin { from, onto } => {
                    tabs(TabsIn::Unpin(from));
                    tabs(TabsIn::Place {
                        tab: from,
                        at: onto,
                    });
                }
                Dropped::Pin { from, onto } => {
                    tabs(TabsIn::Pin {
                        tab: from,
                        relaunch: Relaunch(String::new()),
                    });
                    tabs(TabsIn::Place {
                        tab: from,
                        at: onto,
                    });
                }
                Dropped::Tear(pane) => desk.act(WorkspaceIn::Tear(pane)),
                Dropped::Nothing => {}
            }
        })
    };
    // The pointer left the window with the drag in its hand: it is let go, and nothing happens.
    let cancel = {
        let dnd = dnd.clone();
        use_callback(move |(): ()| {
            let mut over = dnd.over;
            over.set(None);
            dnd.drag.up();
        })
    };
    let moved = {
        let dnd = dnd.clone();
        move |event: PointerEvent| {
            if matches!(dnd.drag.phase(), DragPhase::Idle) {
                return;
            }
            let at = point_of(&event);
            dnd.drag.moved(at);
            let under = dnd.rects.at(at);
            let mut over = dnd.over;
            if *over.peek() != under {
                over.set(under);
            }
        }
    };

    let _ = ui();
    if let Some(link) = &desktop.link {
        hub.publish_to(link);
    }
    let data = hub.sidebar();
    let front = hub.front();
    let bar = hub.bar_data();
    let quick = hub.quick_shown().map(|pane| (pane, hub.bar_spaces()));
    let (title, cwd) = hub.window_parts();
    use_window_title(window_title(&title, cwd.as_deref()));
    let ghost = match dnd.drag.phase() {
        DragPhase::Live { key, at, .. } => Some((ghost_text(&data, key), at)),
        DragPhase::Idle | DragPhase::Pending { .. } => None,
    };
    let now = env();
    // Pinned, the sidebar is the pane's body; hidden, it moves out of the pane (which clips what
    // floats over it) into a zero-width host at the window's edge, where it peeks.
    let side_box = |pinned: Shown| {
        rsx! {
            div {
                class: "temor-side-box",
                onmounted: move |event: MountedEvent| side.on_mounted(event),
                TabSidebar { data: data.clone(), desk: desk.clone(), dnd: dnd.clone(), pinned }
            }
        }
    };
    let sidebar_body = match sidebar() {
        Shown::Visible => side_box(Shown::Visible),
        Shown::Hidden => rsx! {},
    };
    rsx! {
        Ds {
            appearance: now.settings.appearance.appearance(),
            system: now.system,
            tint_alpha: Some(now.tint_alpha()),
            stack: Some(now.material_stack()),
            typeface: Some(now.settings.appearance.typeface()),
            look: look.clone(),
            material: Material::Window,
            extent: RootExtent::Window,
            style { {WINDOW_CSS} }
            PaletteFollow {
                hub: hub.clone(),
                env: ReadSignal::from(env),
                notice,
            }
            div {
                class: "temor-shell",
                onmounted: move |event: MountedEvent| shell.on_mounted(event),
                onpointermove: moved,
                onpointerup: move |_| release.call(()),
                onpointerleave: move |_| cancel.call(()),
                SplitView {
                    label: "Window",
                    panes: vec![
                        SplitPane::new(sidebar_spec(), sidebar_body).shown(sidebar()),
                    ],
                    on_shown: move |(_, shown): (usize, Shown)| sidebar.set(shown),
                    div { class: "temor-window",
                        if let Some(text) = notice() {
                            InlineBanner {
                                severity: Severity::Warn,
                                text,
                                onclose: move |_| notice.set(None),
                            }
                        }
                        TransferAsk { desk: desk.clone() }
                        div { class: "temor-content",
                            if let Some((tab, tree, focus)) = front {
                                TreeView {
                                    key: "{desk.hub.active_space().0}-{tab.0}",
                                    tree,
                                    focus,
                                    desk: desk.clone(),
                                    started,
                                }
                            }
                            if let Some((pane, spaces)) = quick {
                                QuickView { key: "{pane.0}", pane, spaces, desk: desk.clone(), started }
                            }
                        }
                    }
                }
                if sidebar() == Shown::Hidden {
                    div { class: "temor-edge-host", {side_box(Shown::Hidden)} }
                }
            }
            SpaceMenus { desk: desk.clone() }
            if let Some(data) = bar {
                BarView { data, desk: desk.clone() }
            }
            if let Some(((name, sub), at)) = ghost {
                DragGhost { title: name, sub, at }
            }
            AgentSheets { desk: desk.clone() }
        }
    }
}

/// The sidebar's widths, mailo's: 232 to begin with, never narrower than the Spaces foot needs.
fn sidebar_spec() -> PaneSpec {
    PaneSpec {
        preferred: PaneSize::Fixed(Px(SIDEBAR_WIDTH)),
        min: PaneSize::Fixed(Px(SIDEBAR_MIN_WIDTH)),
        ..PaneSpec::SIDEBAR
    }
}

/// The sidebar's width at first, as mailo's.
const SIDEBAR_WIDTH: f32 = 232.0;

/// What the drag ghost says: the tab's title and folder, or the tile's pane.
fn ghost_text(data: &SidebarData, key: DragKey) -> (String, String) {
    let rows = data
        .pinned
        .iter()
        .chain(data.folders.iter().flat_map(|(_, rows)| rows))
        .chain(&data.loose);
    match key {
        DragKey::Tab(tab) => rows
            .filter(|row| row.id == tab)
            .map(|row| (row.title.clone(), row.cwd.clone().unwrap_or_default()))
            .next()
            .unwrap_or_default(),
        DragKey::Tile(pane) => rows
            .flat_map(|row| row.tiles.iter().map(move |tile| (row, tile)))
            .filter(|(_, tile)| tile.pane == pane)
            .map(|(row, tile)| (tile.title.clone(), row.title.clone()))
            .next()
            .unwrap_or_default(),
    }
}
