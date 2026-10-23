//! The live cards of one pane: a command that runs in a terminal of its own, drawn inside its
//! block's card (btop, a dev server, an agent). One state and one step, so the card's icons, its
//! menu and the command bar all go through the same rules. The edge starts and signals the
//! sessions; this only says what should happen and in what order.

use std::collections::BTreeMap;

use crate::vocab::ids::BlockId;

/// A terminal session of the program's own, as the edge names it (the local daemon's session).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SurfaceId(pub u64);

/// How tall a live card draws its terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardSize {
    Compact,
    #[default]
    Normal,
    Tall,
    /// The whole pane, or the whole window.
    Full,
}

impl CardSize {
    /// The next size the card's size icon moves to. Full is left with its own icon.
    pub fn cycled(self) -> Self {
        match self {
            Self::Compact => Self::Normal,
            Self::Normal => Self::Tall,
            Self::Tall | Self::Full => Self::Compact,
        }
    }
}

/// Where a live card's terminal is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// In the card, in the feed.
    Card,
    /// In a small window of its own that floats above the others.
    Mini,
}

/// Whether the program is still there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Life {
    Running,
    /// Asked to stop with an interrupt; hung up if it is still there when the grace is over.
    Stopping,
    /// Gone with this exit status (`None` when it is not known); the last frame stays.
    Ended(Option<i32>),
}

/// One live card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Surface {
    pub session: SurfaceId,
    pub size: CardSize,
    pub placed: Placement,
    pub life: Life,
}

/// The live cards of a pane, by the block they belong to.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cards {
    surfaces: BTreeMap<BlockId, Surface>,
}

impl Cards {
    pub fn get(&self, id: BlockId) -> Option<&Surface> {
        self.surfaces.get(&id)
    }

    pub fn iter(&self) -> impl Iterator<Item = (BlockId, &Surface)> + '_ {
        self.surfaces.iter().map(|(id, surface)| (*id, surface))
    }

    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }
}

/// What changes the cards.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CardIn {
    /// The person asked for the running program of `id` to get a terminal of its own.
    Detach(BlockId),
    /// The edge has moved that program to `session`; its card is live from now on.
    Adopted { id: BlockId, session: SurfaceId },
    /// The size icon.
    CycleSize(BlockId),
    /// A size by name (the menu, full screen).
    SetSize { id: BlockId, size: CardSize },
    /// The mini-window icon.
    Pop(BlockId),
    /// Back from the mini window to the card.
    Dock(BlockId),
    /// The stop icon.
    Terminate(BlockId),
    /// The grace after an interrupt is over.
    GraceOver(BlockId),
    /// The program left.
    Ended { id: BlockId, exit: Option<i32> },
    /// A card of a program that left is put away.
    Dismiss(BlockId),
}

/// What the edge does after a step.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CardOut {
    /// Move the running program of this block to a session of its own, then say
    /// [`CardIn::Adopted`]; the pane gets a new shell.
    Detach(BlockId),
    /// Resize the program's terminal to what the card's size draws.
    Resize { session: SurfaceId, size: CardSize },
    /// Open the mini window for this card's terminal.
    OpenMini { id: BlockId, session: SurfaceId },
    /// Close it.
    CloseMini(BlockId),
    /// Send the program an interrupt (Ctrl-C).
    Interrupt(SurfaceId),
    /// Say [`CardIn::GraceOver`] for this block after a short wait.
    StartGrace(BlockId),
    /// Hang the program up.
    HangUp(SurfaceId),
    /// Keep the card's last frame as it is.
    Freeze(BlockId),
    /// Draw the card again.
    Redraw(BlockId),
}

/// The cards after `input`, and what to do about it.
pub fn card_step(mut state: Cards, input: CardIn) -> (Cards, Vec<CardOut>) {
    let mut outs = Vec::new();
    match input {
        CardIn::Detach(id) => {
            if !state.surfaces.contains_key(&id) {
                outs.push(CardOut::Detach(id));
            }
        }
        CardIn::Adopted { id, session } => {
            state.surfaces.insert(
                id,
                Surface {
                    session,
                    size: CardSize::default(),
                    placed: Placement::Card,
                    life: Life::Running,
                },
            );
            outs.push(CardOut::Redraw(id));
        }
        CardIn::CycleSize(id) => {
            if let Some(next) = state.surfaces.get(&id).map(|surface| surface.size.cycled()) {
                set_size(&mut state, id, next, &mut outs);
            }
        }
        CardIn::SetSize { id, size } => set_size(&mut state, id, size, &mut outs),
        CardIn::Pop(id) => {
            if let Some(surface) = state.surfaces.get_mut(&id)
                && matches!(surface.life, Life::Running | Life::Stopping)
                && surface.placed == Placement::Card
            {
                surface.placed = Placement::Mini;
                outs.push(CardOut::OpenMini {
                    id,
                    session: surface.session,
                });
                outs.push(CardOut::Redraw(id));
            }
        }
        CardIn::Dock(id) => {
            if let Some(surface) = state.surfaces.get_mut(&id)
                && surface.placed == Placement::Mini
            {
                surface.placed = Placement::Card;
                outs.push(CardOut::CloseMini(id));
                outs.push(CardOut::Redraw(id));
            }
        }
        CardIn::Terminate(id) => {
            if let Some(surface) = state.surfaces.get_mut(&id)
                && surface.life == Life::Running
            {
                surface.life = Life::Stopping;
                outs.push(CardOut::Interrupt(surface.session));
                outs.push(CardOut::StartGrace(id));
                outs.push(CardOut::Redraw(id));
            }
        }
        CardIn::GraceOver(id) => {
            if let Some(surface) = state.surfaces.get(&id)
                && surface.life == Life::Stopping
            {
                outs.push(CardOut::HangUp(surface.session));
            }
        }
        CardIn::Ended { id, exit } => {
            if let Some(surface) = state.surfaces.get_mut(&id)
                && !matches!(surface.life, Life::Ended(_))
            {
                surface.life = Life::Ended(exit);
                if surface.placed == Placement::Mini {
                    surface.placed = Placement::Card;
                    outs.push(CardOut::CloseMini(id));
                }
                outs.push(CardOut::Freeze(id));
                outs.push(CardOut::Redraw(id));
            }
        }
        CardIn::Dismiss(id) => {
            if state
                .surfaces
                .get(&id)
                .is_some_and(|surface| matches!(surface.life, Life::Ended(_)))
            {
                state.surfaces.remove(&id);
                outs.push(CardOut::Redraw(id));
            }
        }
    }
    (state, outs)
}

fn set_size(state: &mut Cards, id: BlockId, size: CardSize, outs: &mut Vec<CardOut>) {
    let Some(surface) = state.surfaces.get_mut(&id) else {
        return;
    };
    if matches!(surface.life, Life::Ended(_)) || surface.size == size {
        return;
    }
    surface.size = size;
    outs.push(CardOut::Resize {
        session: surface.session,
        size,
    });
    outs.push(CardOut::Redraw(id));
}
