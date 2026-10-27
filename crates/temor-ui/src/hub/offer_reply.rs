//! The person's answer to the shell offer, and what follows it (PLAN P17): a Yes types one
//! line that makes the inner shell name itself; once the tick has heard the name, the
//! integration written for that shell is typed. Nothing is typed before a Yes.

use temor_client::model::offer::{Line, OfferIn, OfferOut, OfferState, Program, Reply, Screen};
use temor_client::model::vocab::ids::PaneId;

use super::model::Hub;
use temor_client::ssh::InnerShell;

use crate::edge::offer::{load_line, probe_text, remember_never};
use crate::session::Session;
use crate::settings::kept_aside;

impl Hub {
    /// The person answered the offer in `pane`. A Yes types a one-line question into it; the
    /// integration follows once the shell has answered (see [`Hub::offer_tick`]). When the
    /// command line already names the shell (`docker exec -it web zsh`) the integration is typed
    /// at once, with no question to land in the shell's history. Returns what the person should
    /// be told.
    pub fn offer_reply(&mut self, pane: PaneId, reply: Reply) -> Vec<String> {
        let Some(slot) = self.slots.get(&pane).cloned() else {
            return Vec::new();
        };
        let named = self.offer.states.get(&pane).and_then(OfferState::asking);
        let (at, busy, shell) = match slot.borrow().session.as_ref() {
            Some(session) => (
                session.now(),
                Self::offer_busy(session),
                named
                    .zip(session.running_line())
                    .and_then(|(program, command)| {
                        InnerShell::named_in(&command, &program.to_string())
                    }),
            ),
            None => return Vec::new(),
        };
        let (reply, mut notes) = match (reply, busy) {
            (Reply::Yes, true) => (
                Reply::NotNow,
                vec!["Not set up: the program is waiting for an answer.".to_owned()],
            ),
            (reply, _) => (reply, Vec::new()),
        };
        let input = match (reply, shell) {
            (Reply::Yes, Some(shell)) => {
                self.offer.shells.insert(pane, shell);
                OfferIn::YesNamed(at)
            }
            (reply, _) => OfferIn::Reply(reply, at),
        };
        let (_, outs) = self.offer_feed(pane, input);
        notes.extend(self.offer_outs(pane, outs));
        notes
    }

    /// Whether the program is asking something right now: a typed line would answer it.
    fn offer_busy(session: &Session) -> bool {
        session.last_line_kind() == Line::Asking || session.screen_kind() == Screen::Alternate
    }

    pub(super) fn offer_outs(&mut self, pane: PaneId, outs: Vec<OfferOut>) -> Vec<String> {
        let mut notes = Vec::new();
        for out in outs {
            let note = match out {
                OfferOut::Redraw => None,
                OfferOut::Probe => self.offer_type(pane, &probe_text()),
                OfferOut::Load => self.offer_load(pane),
                OfferOut::Unsure => {
                    self.offer.shells.remove(&pane);
                    Some("Not set up: could not tell which shell this is.".to_owned())
                }
                OfferOut::Never(program) => self.offer_never(&program),
            };
            notes.extend(note);
        }
        notes
    }

    /// Types `text` into `pane`, each newline as the Enter key sends it (`\r`): a line editor
    /// in raw mode (nushell's) reads `\n` as Ctrl+J and runs nothing, and a terminal in line
    /// mode turns `\r` back into `\n`.
    fn offer_type(&self, pane: PaneId, text: &str) -> Option<String> {
        let slot = self.slots.get(&pane)?;
        let guard = slot.borrow();
        guard
            .session
            .as_ref()?
            .pane()
            .send(text.replace('\n', "\r").as_bytes());
        None
    }

    /// Types the integration written for the shell the pane named.
    fn offer_load(&mut self, pane: PaneId) -> Option<String> {
        let shell = self.offer.shells.remove(&pane)?;
        match load_line(shell) {
            Ok(line) => self.offer_type(pane, line.as_str()),
            Err(_) => Some("Could not set up blocks in this shell.".to_owned()),
        }
    }

    /// Takes `program` off the list, now and in the settings file.
    fn offer_never(&mut self, program: &Program) -> Option<String> {
        self.offer.list.remove(program);
        match remember_never(&self.offer.settings, program) {
            Ok(kept) => kept.as_deref().map(kept_aside),
            Err(_) => Some("Could not save this choice.".to_owned()),
        }
    }
}
