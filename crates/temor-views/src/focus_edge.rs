//! A condition's rising edge, for what is done once when something comes to hold: a pane that
//! becomes the keyboard pane takes the keyboard, the composer that comes up takes the caret, the
//! tab that comes to the front is scrolled to. Asked again (the window's `regain` moved), it
//! is done again while the condition holds.

use std::cell::Cell;
use std::rc::Rc;

use dioxus::prelude::*;

/// Whether the condition held, and the ask last seen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Edge {
    held: bool,
    asked: u32,
}

impl Edge {
    /// The condition now and the ask now: the edge after, and whether to act (it came to hold,
    /// or it holds and the ask moved).
    pub fn step(self, holds: bool, asked: u32) -> (Edge, bool) {
        let again = self.asked != asked;
        let act = holds && (!self.held || again);
        (Edge { held: holds, asked }, act)
    }

    /// The condition was seen to stop holding outside the effect (the keyboard left).
    pub fn dropped(self) -> Edge {
        Edge {
            held: false,
            ..self
        }
    }
}

/// Runs `then` from an effect each time what `holds` reads (the condition, and the ask) rises
/// (see [`Edge::step`]). The edge is returned, so a handler can drop it.
pub fn use_focus_edge(
    mut holds: impl FnMut() -> (bool, u32) + 'static,
    mut then: impl FnMut() + 'static,
) -> Rc<Cell<Edge>> {
    let edge = use_hook(|| Rc::new(Cell::new(Edge::default())));
    let kept = Rc::clone(&edge);
    use_effect(move || {
        let (now, asked) = holds();
        let (next, act) = kept.get().step(now, asked);
        kept.set(next);
        if act {
            then();
        }
    });
    edge
}

#[cfg(test)]
mod tests {
    use super::Edge;

    #[test]
    fn it_acts_when_the_condition_comes_to_hold_and_when_asked_again() {
        // (holds, asked, act)
        let steps = [
            (false, 0, false),
            (true, 0, true),
            (true, 0, false),
            (true, 1, true),
            (false, 2, false),
            (false, 2, false),
            (true, 2, true),
        ];
        let mut edge = Edge::default();
        for (at, (holds, asked, want)) in steps.into_iter().enumerate() {
            let (next, act) = edge.step(holds, asked);
            assert_eq!(act, want, "step {at}");
            edge = next;
        }
        assert!(edge.dropped().step(true, 2).1, "dropped: it acts again");
    }
}
