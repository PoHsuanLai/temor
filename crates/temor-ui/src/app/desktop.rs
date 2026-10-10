//! Joining the desktop (PLAN §11 P6): before the window opens, the binary asks whether another
//! temor is already running. If so, this launch's request (a new window) is handed over and the
//! launch ends. If not, this one is the first: it serves temor's actions to the agent layer,
//! hears later launches, publishes the dock badge and follows the Tailscale computers, all on the
//! caller's tokio runtime, and gives the window the half of the link it draws from.

use std::sync::Arc;

use temor_desktop::{
    Badge, BadgeCount, Claim, DesktopEnv, DesktopError, Primary, Request, SystemDock,
    SystemInstance, SystemIntents, SystemTailnet,
};
use tokio::runtime::Handle;
use tokio::sync::watch;

use crate::edge::desktop_feed::{LiveTailnet, NoTailnet, TailnetFeed, group_of};
use crate::edge::desktop_port::{Ask, DesktopLink, DesktopPort, desktop_pair};

/// What the window is given of the desktop. Cloned into the window's root context; a window
/// without it (tests, a desktop with no session bus) reads [`Desktop::none`].
#[derive(Clone, Debug)]
pub struct Desktop {
    /// The link the window publishes through and takes requests from; none without a desktop.
    pub link: Option<DesktopLink>,
    /// The Tailscale computers; the group is absent when empty.
    pub tailnet: Arc<dyn TailnetFeed>,
}

impl Desktop {
    /// No desktop services: nothing is published, the Tailscale group is absent.
    pub fn none() -> Desktop {
        Desktop {
            link: None,
            tailnet: Arc::new(NoTailnet),
        }
    }
}

/// How this launch ended up.
#[derive(Debug)]
pub enum Joined {
    /// This launch is the first: open the window with `desktop`. `trouble` says, for the
    /// terminal, why a desktop that has a session bus could not be joined; the window opens anyway.
    First {
        desktop: Desktop,
        trouble: Option<String>,
    },
    /// Another temor has been asked to open a window; this launch has nothing more to do.
    Handed,
}

/// Claims the first-instance name on `env`'s session bus. Starts the services on `runtime`'s
/// threads, which the caller keeps alive until the window closes. `token` is the activation token
/// this launch was started with (`XDG_ACTIVATION_TOKEN`); a later launch passes it on so the
/// running window can raise itself.
// TODO(platform:macos): a second launch raises the window through NSApplication activation.
pub fn join(runtime: &Handle, env: DesktopEnv, request: &Request, token: Option<&str>) -> Joined {
    let claim = runtime.block_on(SystemInstance::new(env.clone()).claim_with(request, token));
    match claim {
        Ok(Claim::Forwarded) => Joined::Handed,
        Ok(Claim::Primary(primary)) => Joined::First {
            desktop: serve(runtime, env, primary),
            trouble: None,
        },
        Err(error) => Joined::First {
            desktop: Desktop::none(),
            trouble: trouble_of(&error),
        },
    }
}

/// A bus that is missing, or a system without one, is not worth telling the person about.
fn trouble_of(error: &DesktopError) -> Option<String> {
    match error {
        DesktopError::NoBus | DesktopError::Unsupported { .. } => None,
        other => Some(other.to_string()),
    }
}

fn serve(runtime: &Handle, env: DesktopEnv, primary: Primary) -> Desktop {
    let (port, link, count) = desktop_pair();
    let tailnet = LiveTailnet::default();
    runtime.spawn(hear_and_serve(env.clone(), primary, port, link.clone()));
    runtime.spawn(show_badge(env.clone(), count));
    runtime.spawn(follow_tailnet(env, tailnet.clone()));
    Desktop {
        link: Some(link),
        tailnet: Arc::new(tailnet),
    }
}

/// Serves the actions and passes later launches to the window, until the window is gone.
async fn hear_and_serve(
    env: DesktopEnv,
    mut primary: Primary,
    port: DesktopPort,
    link: DesktopLink,
) {
    // Kept alive for the life of the task; an agent layer that is not there is not an error.
    let _intents = SystemIntents::serve(&env, Arc::new(port)).await.ok();
    let asks = link.asker();
    while let Some(heard) = primary.next_handover().await {
        if asks.send(Ask::Launch(heard.request, heard.token)).is_err() {
            break;
        }
    }
}

/// What the dock shows for `count` tabs with news: a number, or nothing.
pub(crate) fn badge_of(count: BadgeCount) -> Badge {
    match count.0 {
        0 => Badge::default(),
        n => Badge {
            count: Some(BadgeCount(n)),
            ..Badge::default()
        },
    }
}

/// Shows the dock count each time it changes, and clears a count an earlier run left behind.
async fn show_badge(env: DesktopEnv, mut count: watch::Receiver<BadgeCount>) {
    let Ok(dock) = SystemDock::connect(&env).await else {
        return;
    };
    loop {
        let shown = *count.borrow_and_update();
        if dock.show(&badge_of(shown)).await.is_err() || count.changed().await.is_err() {
            return;
        }
    }
}

/// Keeps `feed` at the account service's list. A refused or failed read leaves the group absent.
async fn follow_tailnet(env: DesktopEnv, feed: LiveTailnet) {
    let Ok(tailnet) = SystemTailnet::connect(&env).await else {
        return;
    };
    // Subscribe before the first read so no change is missed.
    let mut changes = tailnet.changes().await.ok();
    loop {
        feed.set(group_of(tailnet.hosts().await));
        let Some(stream) = changes.as_mut() else {
            return;
        };
        if stream.next().await.is_none() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dock_shows_a_number_or_nothing() {
        assert_eq!(badge_of(BadgeCount(0)), Badge::default());
        let three = badge_of(BadgeCount(3));
        assert_eq!(three.count, Some(BadgeCount(3)));
        assert_eq!(three.progress, None);
    }
}
