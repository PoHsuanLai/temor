//! The calls of protocol version 2 that are not a plain question and answer: a ping, the window's
//! colours, and one part of a streamed write.

use std::time::Duration;

use temor_wire::ids::SessionId;
use temor_wire::message::{
    FileStamp, Palette, PartEnd, RemotePath, Reply, Request, WriteCheck, WriteMode,
};
use temor_wire::units::ByteOffset;

use super::calls::InFlight;
use super::connect::Client;
use crate::error::Error;

impl Client {
    /// Asks the helper whether it is there, and waits at most `within` for the answer
    /// ([`Error::NoAnswer`] when none comes, which the window reports to `link_step` as
    /// `PingTimedOut`). Needs no capability.
    ///
    /// The ping is written to the link first. If the link is so stalled that the write itself
    /// blocks, this call blocks with it, so call it from a thread the window can abandon, and
    /// treat a call that has not returned well after `within` as unanswered too.
    pub fn ping(&self, within: Duration) -> Result<(), Error> {
        let answer = self.call_within(Request::Ping, None, Some(within))?;
        match answer.outcome.map_err(Error::Remote)? {
            Reply::Pong => Ok(()),
            _ => Err(Error::UnexpectedReply),
        }
    }

    /// Sets the colours a session's terminal answers colour queries with, so programs on the host
    /// that ask get the window's.
    pub fn set_palette(&self, session: SessionId, palette: Palette) -> Result<(), Error> {
        self.ack(Request::SetPalette { session, palette })
    }

    /// Sends one part of a streamed write: `bytes` go at `offset` in the file. Parts must come in
    /// order, the first at offset 0. Nothing replaces the file until the part that ends with
    /// [`PartEnd::Last`], which checks `check` and returns the file's new stamp; the parts before
    /// it, and [`PartEnd::Abort`], return `None`.
    pub fn write_part(
        &self,
        path: &RemotePath,
        check: WriteCheck,
        offset: ByteOffset,
        end: PartEnd,
        bytes: &[u8],
    ) -> Result<Option<FileStamp>, Error> {
        let sent = self.send_part(path, check, offset, end, bytes)?;
        self.part_answer(sent, end)
    }

    /// Sends one part of an upload without waiting; [`Client::part_answer`] reads the answer.
    pub(super) fn send_part(
        &self,
        path: &RemotePath,
        check: WriteCheck,
        offset: ByteOffset,
        end: PartEnd,
        bytes: &[u8],
    ) -> Result<InFlight, Error> {
        let request = Request::WriteFile {
            path: path.clone(),
            check,
            mode: WriteMode::Part { offset, end },
        };
        self.send_call(request, Some(bytes))
    }

    /// The answer to a part [`Client::send_part`] sent: the file's stamp after the last part.
    pub(super) fn part_answer(
        &self,
        sent: InFlight,
        end: PartEnd,
    ) -> Result<Option<FileStamp>, Error> {
        let answer = self.answer_of(sent, None)?;
        match (answer.outcome.map_err(Error::Remote)?, end) {
            (Reply::Written(stamp), PartEnd::Last) => Ok(Some(stamp)),
            (Reply::Ack, PartEnd::More | PartEnd::Abort) => Ok(None),
            _ => Err(Error::UnexpectedReply),
        }
    }
}
