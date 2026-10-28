//! A file the program sends with iTerm2's `File=`: it arrives whole, waits for the person, and
//! is written into a staging folder of its own once they say yes.

use std::path::PathBuf;

use temor_transfer::osc1337::{Consented, ReceivedFile};
use temor_transfer::{Consent, RelPath, Root, TransferError};

use super::event::{OfferId, OfferKind, TransferEnd, TransferEvent};
use super::state::{Reaction, Transfers, completeness, ended};

impl Transfers {
    pub(super) fn inline_file(&mut self, payload: &[u8], truncation: bool, out: &mut Reaction) {
        let found = self.reader.feed(payload, completeness(truncation));
        match found {
            Ok(Some(ReceivedFile::NeedsConsent(offer))) => {
                if self.busy() {
                    return;
                }
                let id = self.fresh_offer();
                out.events.push(TransferEvent::Offered {
                    offer: id,
                    kind: OfferKind::Inline {
                        name: offer.name().as_str().to_owned(),
                        size: offer.size().get(),
                    },
                });
                self.inline.insert(id, offer);
            }
            // Pictures meant to be shown in the terminal are not drawn yet, and nothing is saved
            // for them; the rest of a multipart file is still arriving.
            Ok(_) => {}
            Err(err) => out.events.push(TransferEvent::Unreadable {
                why: err.to_string(),
            }),
        }
    }

    pub(super) fn decide_inline(&mut self, offer: OfferId, consent: Consent, out: &mut Reaction) {
        let Some(pending) = self.inline.remove(&offer) else {
            return;
        };
        let Some(consented) = pending.consent(consent) else {
            out.events.push(ended(offer, TransferEnd::Refused));
            return;
        };
        match self.write_inline(&consented) {
            Ok((file, path)) => {
                let size = consented.data().len() as u64;
                out.events.push(TransferEvent::Started {
                    offer,
                    file: file.clone(),
                    name: file.clone(),
                    size: Some(size),
                });
                out.events.push(TransferEvent::Saved {
                    offer,
                    file,
                    path,
                    size,
                });
                out.events.push(ended(offer, TransferEnd::Finished));
            }
            Err(_) => out.events.push(ended(offer, TransferEnd::Failed)),
        }
    }

    /// Writes the file under its own staging folder; returns its name and where it is.
    fn write_inline(&mut self, file: &Consented) -> Result<(String, PathBuf), TransferError> {
        let dir = self.new_dir().map_err(|source| TransferError::Io {
            path: self.setup.staging.clone(),
            source,
        })?;
        let root = Root::open(&dir)?;
        let rel = RelPath::parse(file.name().as_str()).or_else(|_| RelPath::parse("download"))?;
        root.create_file(&rel)?;
        root.append(&rel, file.data())?;
        root.finish(&rel, None)?;
        Ok((rel.leaf().to_owned(), dir.join(rel.to_path_buf())))
    }
}
