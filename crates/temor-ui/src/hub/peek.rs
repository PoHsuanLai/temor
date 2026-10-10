//! The peek card's worker in the hub (PLAN §10): started when the first card is asked for, shared
//! by every pane (the pointer is in one pane at a time). The view's pure step (`peek_step`) decides
//! when to ask; this looks at the file and hands the card back.

use std::cell::OnceCell;
use std::path::Path;
use std::sync::Arc;

use anyview_peek::AnyPeeked;

use super::model::HubRef;
use crate::edge::peek_look::PeekLook;
use crate::edge::peek_mail::{Taken, Ticket};

/// What the hub keeps for the peek.
#[derive(Default)]
pub(super) struct PeekHub {
    look: OnceCell<PeekLook>,
}

impl HubRef {
    /// Asks for the peek card of `file`; none when the file has no absolute path.
    pub fn peek_ask(&self, file: &Path) -> Option<Ticket> {
        let hub = self.0.borrow();
        hub.peek.look.get_or_init(PeekLook::start).ask(file)
    }

    /// The card for `ticket`, once it is made.
    pub fn peek_take(&self, ticket: Ticket) -> Taken<Arc<AnyPeeked>> {
        match self.0.borrow().peek.look.get() {
            Some(look) => look.take(ticket),
            None => Taken::Stale,
        }
    }

    /// The card is no longer wanted.
    pub fn peek_cancel(&self) {
        if let Some(look) = self.0.borrow().peek.look.get() {
            look.cancel();
        }
    }
}
