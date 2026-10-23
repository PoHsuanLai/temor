//! Opening or reattaching one pane's session on a connection (PLAN §9b): the call sequence, the
//! screen handed to the pane, and the typing held while the link was down sent ahead of any new (a piece cut off mid-send is
//! dropped, never repeated).

use std::sync::Arc;

use temor_remote::Error;
use temor_wire::ids::SessionId;
use temor_wire::message::RemoteError;

use super::remote_adopt::{Adoption, adoption};
use super::remote_calls::{attach, input, open, resize, sessions, set_palette};
use super::remote_conn::Conn;
use super::remote_event::{AttachFault, Ticket, Wanted};
use super::remote_link::{SharedTarget, lock};
use super::remote_route::Router;

/// Opens or reattaches the session `ticket` names on `conn`, hands the pane its screen, and
/// points the pane's input at it.
pub fn attach_pane(
    conn: &Arc<Conn>,
    router: &Router,
    ticket: Ticket,
) -> Result<SessionId, AttachFault> {
    let client = conn.client();
    let (size, cell) = {
        let target = lock(&ticket.target);
        (target.size, target.cell)
    };
    let palette = ticket.palette;
    let opened = match &ticket.wanted {
        Wanted::Fresh { cwd, env, shell } => open(
            client,
            (cwd.as_str(), env.as_slice(), shell.as_deref()),
            (size, cell),
            palette,
        ),
        Wanted::Again(session) => Ok(*session),
        Wanted::Resume {
            session,
            cwd,
            env,
            shell,
        } => sessions(client).and_then(|held| match adoption(*session, &held) {
            Adoption::Adopt => Ok(*session),
            Adoption::OpenNew => open(
                client,
                (cwd.as_str(), env.as_slice(), shell.as_deref()),
                (size, cell),
                palette,
            ),
        }),
    };
    let attached = opened.and_then(|session| {
        // The screen the host sends back is at the size the window has now.
        let _ = resize(client, session, (size, cell));
        let shown = attach(client, session)?;
        // The colours may have changed since the session was last attached.
        let _ = set_palette(client, session, palette);
        Ok((session, shown))
    });
    match attached {
        Ok((session, shown)) => {
            router.attach(session, ticket.feeder, shown.screen());
            publish(&ticket.target, conn, session);
            Ok(session)
        }
        Err(Error::Remote(RemoteError::NoSuchSession { .. })) => Err(AttachFault::Gone),
        Err(_) => Err(AttachFault::Failed),
    }
}

/// Points the pane's input at `session`. What was typed while there was no link goes first, in
/// order; the target is published only once nothing more is held, so a key typed meanwhile
/// cannot overtake it.
fn publish(target: &SharedTarget, conn: &Arc<Conn>, session: SessionId) {
    loop {
        let held = {
            let mut target = lock(target);
            if target.held.is_empty() {
                // Empty, but taking it also clears the note that typing was dropped.
                let _ = target.held.take();
                target.conn = Some(Arc::clone(conn));
                target.session = Some(session);
                return;
            }
            target.held.take()
        };
        if input(conn.client(), session, &held).is_err() {
            // The link went again mid-send: that piece may be half received, so it is not sent
            // twice. What was typed since waits for the next attach, still in order.
            let mut target = lock(target);
            let later = target.held.take();
            target.held.push(&later);
            target.held.lose_in_flight();
            return;
        }
    }
}
