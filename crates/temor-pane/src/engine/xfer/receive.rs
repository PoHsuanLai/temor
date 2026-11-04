//! Files the program sends with kitty's protocol: one [`ReceiveSession`] per offer, its writes
//! carried out under the offer's own staging folder. The person's answer arrives later as a
//! command, so the machine waits in `Asking` meanwhile (and has already said `STARTED`).

use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;

use temor_transfer::kitty::{
    Effect, ErrorCode, FileId, FileType, Input, KittyId, Message, SavedFile, TransferEnd,
    WriteKind, WriteOp, apply_write, wire_code,
};
use temor_transfer::{Consent, HostName, Origin, Root};

use super::event::{OfferId, OfferKind, TransferEvent};
use super::state::{Reaction, Transfers, ended};

/// Progress is reported at most this often, in bytes.
const REPORT_EVERY: u64 = 256 * 1024;

/// One kitty transfer in flight.
#[derive(Debug)]
pub struct Receive {
    offer: OfferId,
    session: Option<temor_transfer::kitty::ReceiveSession>,
    /// The staging folder and its root, once the person has said yes.
    dir: Option<(PathBuf, Root)>,
    /// The sizes the sender announced, by file.
    sizes: BTreeMap<FileId, u64>,
    /// Bytes received per file, and what was last reported.
    seen: BTreeMap<String, (u64, u64)>,
}

type Work = VecDeque<(Input, Option<FileId>)>;

impl Transfers {
    pub(super) fn begin_receive(&mut self, message: Message, out: &mut Reaction) {
        if self.busy() {
            out.refuse(message.id, ErrorCode::Eperm, "too many transfers at once");
            return;
        }
        let offer = self.fresh_offer();
        // Only a label: the window decides who the offer is from.
        let origin = Origin::Host {
            host: HostName("".into()),
        };
        let id = message.id.clone();
        let mut receive = Receive {
            offer,
            session: Some(temor_transfer::kitty::ReceiveSession::new(
                origin,
                self.setup.limits,
            )),
            dir: None,
            sizes: BTreeMap::new(),
            seen: BTreeMap::new(),
        };
        let ended = receive.run(VecDeque::from([(Input::Frame(message), None)]), out);
        self.finish_receive(&id, receive, ended, out);
    }

    pub(super) fn receive_frame(&mut self, message: Message, out: &mut Reaction) {
        let id = message.id.clone();
        let fid = message.file.clone();
        self.step_receive(&id, Input::Frame(message), fid, out);
    }

    pub(super) fn receive_malformed(&mut self, id: &KittyId, out: &mut Reaction) {
        self.step_receive(id, Input::Malformed { id: id.clone() }, None, out);
    }

    pub(super) fn receive_aborted(&mut self, id: &KittyId, out: &mut Reaction) {
        self.step_receive(id, Input::Aborted, None, out);
    }

    pub(super) fn receive_of(&self, offer: OfferId) -> Option<KittyId> {
        self.receives
            .iter()
            .find(|(_, receive)| receive.offer == offer)
            .map(|(id, _)| id.clone())
    }

    pub(super) fn decide_receive(&mut self, id: &KittyId, consent: Consent, out: &mut Reaction) {
        let staged = match consent {
            Consent::Allow => self.stage(),
            Consent::Refuse => None,
        };
        let Some(mut receive) = self.receives.remove(id) else {
            return;
        };
        let mut work = Work::new();
        work.push_back((Input::Decided(consent), None));
        match (consent, staged) {
            (Consent::Allow, Some(dir)) => receive.dir = Some(dir),
            (Consent::Allow, None) => {
                // No room to put the file: the program is told the transfer broke.
                let error = ErrorCode::Eio;
                let op = WriteKind::MakeDir;
                work.push_back((
                    Input::WriteFailed {
                        fid: None,
                        op,
                        error,
                    },
                    None,
                ));
            }
            (Consent::Refuse, _) => {}
        }
        let ended = receive.run(work, out);
        self.finish_receive(id, receive, ended, out);
    }

    fn stage(&mut self) -> Option<(PathBuf, Root)> {
        let dir = self.new_dir().ok()?;
        let root = Root::open(&dir).ok()?;
        Some((dir, root))
    }

    fn step_receive(
        &mut self,
        id: &KittyId,
        input: Input,
        fid: Option<FileId>,
        out: &mut Reaction,
    ) {
        let Some(mut receive) = self.receives.remove(id) else {
            return;
        };
        let ended = receive.run(VecDeque::from([(input, fid)]), out);
        self.finish_receive(id, receive, ended, out);
    }

    /// Keeps the transfer, or closes it when its machine ended.
    fn finish_receive(
        &mut self,
        id: &KittyId,
        receive: Receive,
        ended_why: Option<TransferEnd>,
        out: &mut Reaction,
    ) {
        match ended_why {
            None => {
                self.receives.insert(id.clone(), receive);
            }
            Some(why) => {
                if let Some((dir, _)) = &receive.dir {
                    // Empty once the window has taken the files; otherwise it stays for it.
                    let _ = std::fs::remove_dir(dir);
                }
                out.events.push(ended(receive.offer, why));
            }
        }
    }
}

impl Receive {
    /// Runs inputs through the machine until nothing is left, carrying out its effects. Returns
    /// how the transfer ended, if it did.
    fn run(&mut self, mut work: Work, out: &mut Reaction) -> Option<TransferEnd> {
        let mut over = None;
        while let Some((input, fid)) = work.pop_front() {
            self.note_size(&input);
            let Some(session) = self.session.take() else {
                break;
            };
            let (next, effects) = session.step(input);
            self.session = Some(next);
            for effect in effects {
                match effect {
                    Effect::AskConsent(_) => out.events.push(TransferEvent::Offered {
                        offer: self.offer,
                        kind: OfferKind::Files,
                    }),
                    Effect::Reply(reply) => out.reply(&reply),
                    Effect::Write(op) => self.write(&op, fid.clone(), &mut work, out),
                    Effect::Saved(saved) => self.saved(&saved, out),
                    Effect::Ended(why) => over = Some(why),
                    _ => {}
                }
            }
        }
        over
    }

    fn note_size(&mut self, input: &Input) {
        if let Input::Frame(message) = input
            && let (Some(fid), Some(size)) = (&message.file, message.size)
            && message.file_type != Some(FileType::Directory)
        {
            self.sizes.insert(fid.clone(), size.get());
        }
    }

    /// Performs one write and queues what the machine must hear back (the same contract the
    /// blocking driver keeps).
    fn write(&mut self, op: &WriteOp, fid: Option<FileId>, work: &mut Work, out: &mut Reaction) {
        let result = match &self.dir {
            Some((_, root)) => apply_write(root, op).map_err(|err| wire_code(&err)),
            None => Err(ErrorCode::Eio),
        };
        match result {
            Ok(()) => self.wrote(op, fid, work, out),
            Err(error) => {
                let op = match op {
                    WriteOp::MakeDir(_) => WriteKind::MakeDir,
                    WriteOp::CreateFile(_) => WriteKind::Create,
                    WriteOp::Append { .. } => WriteKind::Append,
                    WriteOp::Finish { .. } => WriteKind::Finish,
                    WriteOp::Discard(_) => WriteKind::Discard,
                    // A write a later temor-transfer adds fails as a discard: nothing is kept.
                    _ => WriteKind::Discard,
                };
                work.push_back((Input::WriteFailed { fid, op, error }, None));
            }
        }
    }

    fn wrote(&mut self, op: &WriteOp, fid: Option<FileId>, work: &mut Work, out: &mut Reaction) {
        match op {
            WriteOp::CreateFile(rel) => {
                let size = fid.as_ref().and_then(|fid| self.sizes.get(fid)).copied();
                out.events.push(TransferEvent::Started {
                    offer: self.offer,
                    file: rel.components().join("/"),
                    name: rel.leaf().to_owned(),
                    size,
                });
            }
            WriteOp::Append { path, data } => {
                let file = path.components().join("/");
                let entry = self.seen.entry(file.clone()).or_default();
                entry.0 += data.len() as u64;
                if entry.0 - entry.1 >= REPORT_EVERY {
                    entry.1 = entry.0;
                    out.events.push(TransferEvent::Progress {
                        offer: self.offer,
                        file,
                        received: entry.0,
                    });
                }
            }
            WriteOp::Finish { .. } => {
                if let Some(fid) = fid {
                    work.push_back((Input::WriteDone { fid: fid.clone() }, Some(fid)));
                }
            }
            _ => {}
        }
    }

    fn saved(&self, saved: &SavedFile, out: &mut Reaction) {
        let Some((dir, _)) = &self.dir else {
            return;
        };
        out.events.push(TransferEvent::Saved {
            offer: self.offer,
            file: saved.path.components().join("/"),
            path: dir.join(saved.path.to_path_buf()),
            size: saved.size.get(),
        });
    }
}
