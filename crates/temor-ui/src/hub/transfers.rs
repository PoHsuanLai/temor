//! What a pane's transfer events mean for the shelf (PLAN §9): an offer asks the person once per
//! stream (`transfer::offer`, then the remembered answer), a file that starts arriving gets a row
//! (`shelf_step(Incoming)`), and a file that is whole is placed in Downloads under a name nobody
//! holds (`Place`, answered with `NameTaken` until one is free).

use std::path::Path;

use temor_core::remote::Grant;
use temor_core::shelf::{FailWhy, Incoming, ShelfIn, ShelfOut, Source};
use temor_core::transfer::{Origin, TransferOut, offer};
use temor_core::vocab::ids::{Bytes, DownloadId, PaneId};
use temor_core::vocab::text::HostName;
use temor_pane::{OfferId, OfferKind, TransferEnd, TransferEvent};
use temor_transfer::Consent;

use super::model::Hub;
use super::shelf::{Ask, AskKind, Live};
use crate::edge::Clock;
use crate::edge::shelf_files::{PlaceError, place};

/// The most names tried for one file before it is given up on.
const MOST_NAMES: u32 = 10_000;

impl Hub {
    /// Reads a batch of transfer events from the panes. Returns what the person should be told.
    pub fn transfers(&mut self, events: Vec<(PaneId, TransferEvent)>) -> Vec<String> {
        for (pane, event) in events {
            match event {
                TransferEvent::Offered { offer, kind } => self.offered(pane, offer, &kind),
                TransferEvent::Wanted { offer, name } => self.wanted(pane, offer, name),
                TransferEvent::Started {
                    offer,
                    file,
                    name,
                    size,
                } => self.started(pane, offer, file, &name, size),
                TransferEvent::Progress {
                    offer,
                    file,
                    received,
                } => {
                    if let Some(id) = self.shelf.files.get(&(pane, offer, file)).copied() {
                        let received = Bytes(received);
                        self.shelf.step(ShelfIn::Progress { id, received });
                    }
                }
                TransferEvent::Saved {
                    offer,
                    file,
                    path,
                    size,
                } => self.saved(pane, offer, file, &path, size),
                TransferEvent::Ended { offer, how } => self.ended(pane, offer, how),
                TransferEvent::Unreadable { .. } => self
                    .notices
                    .push("A file the program sent could not be read".to_owned()),
                // An event a later pane adds has nothing to show yet.
                _ => {}
            }
        }
        std::mem::take(&mut self.notices)
    }

    /// Where a pane's files come from. A pane on a host (SSH, PLAN §9b) will say so here.
    fn origin_of(&self, pane: PaneId) -> Option<(Origin, String)> {
        // TODO(ssh): `Origin::Remote` for a pane that runs a host session.
        let tab = self.book.ws.tab_of(pane)?;
        let title = self
            .book
            .ws
            .tabs
            .tab(tab)
            .map(|t| t.title.as_str().to_owned());
        let from = match title.filter(|title| !title.is_empty()) {
            Some(title) => format!("A program in “{title}”"),
            None => "A program in this window".to_owned(),
        };
        Some((Origin::LocalStream { tab }, from))
    }

    fn offered(&mut self, pane: PaneId, id: OfferId, kind: &OfferKind) {
        let Some((origin, from)) = self.origin_of(pane) else {
            return;
        };
        let stream = stream_of(&origin, pane);
        let grant = self
            .shelf
            .grants
            .get(&stream)
            .copied()
            .unwrap_or(Grant::Ask);
        let source = match kind {
            OfferKind::Inline { .. } => Source::Osc1337,
            _ => Source::Kitty,
        };
        self.shelf.live.insert(
            (pane, id),
            Live {
                source,
                origin: origin.clone(),
            },
        );
        let (_, outs) = offer(&origin, grant);
        if outs.contains(&TransferOut::Accept) {
            self.decide(pane, id, Consent::Allow);
        } else if outs.contains(&TransferOut::Refuse) {
            self.decide(pane, id, Consent::Refuse);
        } else {
            let kind = match kind {
                OfferKind::Inline { name, size } => AskKind::File {
                    name: name.clone(),
                    size: *size,
                },
                _ => AskKind::Files,
            };
            self.shelf.asks.push_back(Ask {
                pane,
                offer: id,
                kind,
                from,
                stream,
            });
        }
    }

    /// A request for a file is asked about every time: it is not remembered.
    fn wanted(&mut self, pane: PaneId, offer: OfferId, name: Option<String>) {
        let Some((origin, from)) = self.origin_of(pane) else {
            return;
        };
        self.shelf.asks.push_back(Ask {
            pane,
            offer,
            kind: AskKind::Request { name },
            from,
            stream: stream_of(&origin, pane),
        });
    }

    fn started(
        &mut self,
        pane: PaneId,
        offer: OfferId,
        file: String,
        name: &str,
        size: Option<u64>,
    ) {
        let Some(live) = self.shelf.live.get(&(pane, offer)).cloned() else {
            return;
        };
        let outs = self.shelf.step(ShelfIn::Incoming(Incoming {
            source: live.source,
            origin: live.origin,
            name: name.to_owned(),
            size: size.map(Bytes),
            at: self.shelf.clock.now(),
        }));
        let announced = outs.iter().find_map(|out| match out {
            ShelfOut::Announce(id) => Some(*id),
            _ => None,
        });
        if let Some(id) = announced {
            self.shelf.files.insert((pane, offer, file), id);
        }
    }

    fn saved(&mut self, pane: PaneId, offer: OfferId, file: String, staged: &Path, size: u64) {
        let Some(id) = self.shelf.files.remove(&(pane, offer, file)) else {
            // Nothing on the shelf is waiting for it (the row was cleared): drop the copy.
            let _ = std::fs::remove_file(staged);
            return;
        };
        for _ in 0..MOST_NAMES {
            let Some(item) = self.shelf.shelf.get(id) else {
                let _ = std::fs::remove_file(staged);
                return;
            };
            let dest = std::path::PathBuf::from(item.path.as_str());
            match place(staged, &dest) {
                Ok(()) => return self.placed(id, size),
                Err(PlaceError::Taken) => {
                    self.shelf.step(ShelfIn::NameTaken { id });
                }
                Err(PlaceError::Io(_)) => break,
            }
        }
        let _ = std::fs::remove_file(staged);
        self.shelf.step(ShelfIn::Failed {
            id,
            why: FailWhy::Unwritable,
        });
    }

    fn placed(&mut self, id: DownloadId, size: u64) {
        self.shelf.step(ShelfIn::Done {
            id,
            size: Bytes(size),
        });
        self.shelf.arrived(id);
        self.notices.extend(self.shelf.save());
    }

    fn ended(&mut self, pane: PaneId, offer: OfferId, how: TransferEnd) {
        let why = match how {
            TransferEnd::Refused => FailWhy::Refused,
            _ => FailWhy::Broken,
        };
        let unfinished: Vec<_> = self
            .shelf
            .files
            .keys()
            .filter(|(p, o, _)| (*p, *o) == (pane, offer))
            .cloned()
            .collect();
        for key in unfinished {
            if let Some(id) = self.shelf.files.remove(&key) {
                self.shelf.step(ShelfIn::Failed { id, why });
            }
        }
        self.shelf.live.remove(&(pane, offer));
        self.shelf
            .asks
            .retain(|ask| (ask.pane, ask.offer) != (pane, offer));
    }
}

/// What an answer is remembered for: a host, or one pane's own output.
fn stream_of(origin: &Origin, pane: PaneId) -> String {
    match origin {
        Origin::Remote {
            host: HostName(host),
            ..
        } => format!("host:{host}"),
        _ => format!("pane:{}", pane.0),
    }
}
