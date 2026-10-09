//! `context_step`: taking chips off and putting them back, then sending exactly what is left.

use super::model::{ChipId, ContextCard, Removal, SentChip, SentList};
use crate::vocab::ai::AiSwitch;

/// Where the card stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextPhase {
    /// Shown, waiting for the person.
    Editing(ContextCard),
    /// Sent; the list is what was sent.
    Sent(SentList),
    /// Dismissed; nothing was sent.
    Cancelled,
}

/// What the person does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextIn {
    /// Take a chip off.
    Remove(ChipId),
    /// Put a removed chip back.
    AddBack(ChipId),
    /// Send what the card shows. Refused while AI is switched off.
    Send(AiSwitch),
    /// Close the card without sending.
    Cancel,
}

/// What follows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextOut {
    /// Send this to the model, and store it with the request's log record.
    Send(SentList),
}

/// The card after `input`. Only `Send` produces output; a fixed chip cannot be removed; nothing
/// changes once the card is sent or cancelled.
pub fn context_step(phase: ContextPhase, input: ContextIn) -> (ContextPhase, Vec<ContextOut>) {
    let ContextPhase::Editing(mut card) = phase else {
        return (phase, vec![]);
    };
    match input {
        ContextIn::Remove(id) => {
            if let Some(at) = card
                .chips
                .iter()
                .position(|c| c.id == id && c.removal == Removal::Removable)
            {
                let chip = card.chips.remove(at);
                card.removed.push(chip);
            }
        }
        ContextIn::AddBack(id) => {
            if let Some(at) = card.removed.iter().position(|c| c.id == id) {
                let chip = card.removed.remove(at);
                // Back in its original place: before the first chip with a larger id.
                let to = card
                    .chips
                    .iter()
                    .position(|c| c.id > chip.id)
                    .unwrap_or(card.chips.len());
                card.chips.insert(to, chip);
            }
        }
        ContextIn::Send(AiSwitch::Off) => {}
        ContextIn::Send(AiSwitch::On) => {
            let list = sent_list(&card);
            return (
                ContextPhase::Sent(list.clone()),
                vec![ContextOut::Send(list)],
            );
        }
        ContextIn::Cancel => return (ContextPhase::Cancelled, vec![]),
    }
    (ContextPhase::Editing(card), vec![])
}

fn sent_list(card: &ContextCard) -> SentList {
    SentList {
        entry: card.entry,
        chips: card
            .chips
            .iter()
            .map(|c| SentChip {
                kind: c.kind,
                label: c.label.clone(),
                size: c.size(),
                text: c.text.clone(),
            })
            .collect(),
        total: card.total(),
    }
}
