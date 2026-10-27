//! Reading a pane's text and typing into it for the desktop services. Everything a terminal
//! printed is somebody else's words: it goes out as text and is never interpreted here.

use temor_client::desktop::{CommandRef, LineRange, PaneRef, PortRefusal};
use temor_client::model::vocab::ids::BlockId;
use temor_client::model::vocab::text::CommandText;

use super::model::Hub;
use super::typing::{Submit, type_command};
use crate::session::Session;

/// The most rows one read returns.
const MOST_ROWS: u32 = 10_000;

impl Hub {
    /// Runs `read` on the pane's live session.
    fn with_session<T>(
        &self,
        pane: PaneRef,
        read: impl FnOnce(&Session) -> Result<T, PortRefusal>,
    ) -> Result<T, PortRefusal> {
        let id = self.known_pane(pane)?;
        let slot = self.slots.get(&id).ok_or(PortRefusal::NoSuchPane)?;
        let slot = slot.borrow();
        let session = slot
            .session
            .as_ref()
            .ok_or_else(|| PortRefusal::Failed("that pane has ended".to_owned()))?;
        read(session)
    }

    /// Rows `range` of the pane's text, counted from the top of its history.
    pub(super) fn read_rows(&self, pane: PaneRef, range: LineRange) -> Result<String, PortRefusal> {
        self.with_session(pane, |session| {
            Ok(session.pane().with_term(|term| {
                let total = term.screen_rows().unwrap_or(0);
                let end = range
                    .from
                    .saturating_add(range.count.min(MOST_ROWS))
                    .min(total);
                (range.from..end)
                    .map(|row| {
                        term.row_text(row)
                            .map(|line| line.text().trim_end().to_owned())
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }))
        })
    }

    /// One finished command's output.
    pub(super) fn read_output(
        &self,
        pane: PaneRef,
        command: CommandRef,
    ) -> Result<String, PortRefusal> {
        let id = BlockId(command.0);
        self.with_session(pane, |session| {
            session
                .block_command(id)
                .ok_or(PortRefusal::NoSuchCommand)?;
            if !session.block_finished(id) {
                return Err(PortRefusal::Failed(
                    "that command is still running".to_owned(),
                ));
            }
            session.block_output(id).ok_or_else(|| {
                PortRefusal::Failed("that command's output is no longer kept".to_owned())
            })
        })
    }

    /// Types `line` and Enter into the pane, as the person would. A line with a line break or
    /// another control character is refused: it would run more than the one command asked for.
    pub(super) fn type_line(&self, pane: PaneRef, line: &str) -> Result<(), PortRefusal> {
        let line = CommandText(line.to_owned());
        if !line.typable() {
            return Err(PortRefusal::Failed(
                "a command must be one line of plain text".to_owned(),
            ));
        }
        self.with_session(pane, |session| {
            type_command(session, &line, Submit::Enter);
            Ok(())
        })
    }
}
