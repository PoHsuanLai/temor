//! Where the list scrolls by itself, as one machine: it keeps to its bottom as it grows until the
//! person scrolls back, a new command takes it back there, and a block picked from the keyboard
//! or the find bar's match is scrolled to once. The pane hands it what it drew and each move of
//! the offset; what comes out is where to scroll.

use ds_core::geometry::scroll::{Scroll, ScrollSpan};
use temor_client::model::vocab::ids::BlockId;

/// The room a revealed find match keeps above it, for the find bar over the list's top.
const FIND_ROOM: f32 = 64.0;

/// What the list follows, and what it saw last.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Follow {
    /// Whether it keeps to its bottom as it grows.
    on: bool,
    /// The offset, and the list's own bottom, as last seen.
    seen: Option<(f32, f32)>,
    /// The list's height as last drawn.
    total: f32,
    /// The list's last block, as last drawn: a new one takes it back to its bottom.
    newest: Option<BlockId>,
    /// The block last asked to be shown.
    revealed: Option<BlockId>,
    /// The find match last asked to be shown.
    found: Option<u32>,
}

impl Default for Follow {
    fn default() -> Self {
        Self {
            on: true,
            seen: None,
            total: 0.0,
            newest: None,
            revealed: None,
            found: None,
        }
    }
}

/// What the list was drawn with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Drawn {
    pub scroll: Scroll,
    /// The list's height.
    pub total: f32,
    /// Its last block.
    pub newest: Option<BlockId>,
    /// The block to show, and its card's top and bottom.
    pub reveal: Option<BlockId>,
    pub reveal_span: Option<(f32, f32)>,
    /// The find match to show (a row), and that row's top.
    pub found: Option<u32>,
    pub found_top: Option<f32>,
    /// A row's height.
    pub row: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum FollowIn {
    /// The list was drawn.
    Drawn(Drawn),
    /// The offset moved, by the wheel or the list itself.
    Moved(Scroll),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum FollowOut {
    /// Scroll to this offset.
    To(f32),
    /// Bring this span into view.
    Reveal(ScrollSpan),
}

impl Follow {
    pub(super) fn step(&mut self, input: FollowIn) -> Vec<FollowOut> {
        match input {
            FollowIn::Moved(scroll) => self.keep_bottom(scroll, self.total).into_iter().collect(),
            FollowIn::Drawn(drawn) => self.drawn(drawn),
        }
    }

    fn drawn(&mut self, drawn: Drawn) -> Vec<FollowOut> {
        let mut out = Vec::new();
        if self.newest != drawn.newest {
            self.newest = drawn.newest;
            self.on = true;
        }
        if self.revealed != drawn.reveal {
            self.revealed = drawn.reveal;
            if let (Some(_), Some((top, bottom))) = (drawn.reveal, drawn.reveal_span) {
                self.on = false;
                let length = (bottom - top).min(drawn.scroll.viewport.0.max(1.0));
                out.push(FollowOut::Reveal(ScrollSpan { start: top, length }));
            }
        }
        if self.found != drawn.found {
            self.found = drawn.found;
            if let (Some(_), Some(top)) = (drawn.found, drawn.found_top) {
                self.on = false;
                out.push(FollowOut::Reveal(ScrollSpan {
                    start: top - FIND_ROOM,
                    length: FIND_ROOM + drawn.row * 2.0,
                }));
            }
        }
        out.extend(self.keep_bottom(drawn.scroll, drawn.total));
        self.total = drawn.total;
        out
    }

    /// Keeps the list to its bottom while it follows it, and in range when it shrinks. When the
    /// offset moves the list follows only from its bottom: a wheel back stops it, a wheel down to
    /// the end, or the list's own move there, starts it again.
    fn keep_bottom(&mut self, scroll: Scroll, total: f32) -> Option<FollowOut> {
        if scroll.viewport.0 <= 0.0 {
            return None;
        }
        let offset = scroll.offset.0;
        // The scroller's own bottom, from the content it measured: it can be reached.
        let reach = scroll.max_offset().0;
        // The list's own bottom: the scroller's measure lags a layout behind it.
        let bottom = (total - scroll.viewport.0).max(0.0);
        let (was, was_bottom) = self.seen.unwrap_or((offset, bottom));
        self.seen = Some((offset, bottom));
        if (offset - was).abs() > 0.5 {
            self.on = offset >= reach.min(bottom) - 1.0;
        }
        // A list that shrank (a fold) under an offset past its new end goes to that end.
        if bottom < was_bottom - 0.5 && offset > bottom + 0.5 {
            Some(FollowOut::To(bottom))
        } else if self.on && offset < reach - 0.5 {
            Some(FollowOut::To(reach))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Drawn, Follow, FollowIn, FollowOut};
    use ds_core::geometry::scroll::{Scroll, ScrollSpan};
    use ds_core::geometry::units::Px;
    use temor_client::model::vocab::ids::BlockId;

    const VIEW: f32 = 100.0;

    fn scroll(offset: f32, content: f32) -> Scroll {
        Scroll {
            offset: Px(offset),
            viewport: Px(VIEW),
            content: Px(content),
        }
    }

    fn drawn(offset: f32, total: f32, newest: u64) -> Drawn {
        Drawn {
            scroll: scroll(offset, total),
            total,
            newest: Some(BlockId(newest)),
            reveal: None,
            reveal_span: None,
            found: None,
            found_top: None,
            row: 16.0,
        }
    }

    #[test]
    fn a_list_that_grows_is_kept_to_its_bottom_until_scrolled_back() {
        let mut follow = Follow::default();
        assert_eq!(follow.step(FollowIn::Drawn(drawn(0.0, 100.0, 1))), vec![]);
        // It grew: to the bottom.
        assert_eq!(
            follow.step(FollowIn::Drawn(drawn(0.0, 300.0, 1))),
            vec![FollowOut::To(200.0)]
        );
        assert_eq!(follow.step(FollowIn::Moved(scroll(200.0, 300.0))), vec![]);
        // The wheel back: it stops following, and growing does not move it.
        assert_eq!(follow.step(FollowIn::Moved(scroll(120.0, 300.0))), vec![]);
        assert_eq!(follow.step(FollowIn::Drawn(drawn(120.0, 400.0, 1))), vec![]);
        // Down to the end again: it follows again.
        assert_eq!(follow.step(FollowIn::Moved(scroll(300.0, 400.0))), vec![]);
        assert_eq!(
            follow.step(FollowIn::Drawn(drawn(300.0, 500.0, 1))),
            vec![FollowOut::To(400.0)]
        );
    }

    #[test]
    fn a_new_command_takes_a_scrolled_back_list_to_its_bottom() {
        let mut follow = Follow::default();
        follow.step(FollowIn::Drawn(drawn(200.0, 300.0, 1)));
        follow.step(FollowIn::Moved(scroll(50.0, 300.0)));
        assert_eq!(follow.step(FollowIn::Drawn(drawn(50.0, 300.0, 1))), vec![]);
        assert_eq!(
            follow.step(FollowIn::Drawn(drawn(50.0, 350.0, 2))),
            vec![FollowOut::To(250.0)]
        );
    }

    #[test]
    fn a_list_that_shrank_under_its_offset_goes_to_its_new_end() {
        let mut follow = Follow::default();
        follow.step(FollowIn::Drawn(drawn(400.0, 500.0, 1)));
        follow.step(FollowIn::Moved(scroll(300.0, 500.0)));
        // A fold: the list is 200 high, its offset still 300 in a scroller that has not measured.
        assert_eq!(
            follow.step(FollowIn::Drawn(Drawn {
                scroll: scroll(300.0, 500.0),
                ..drawn(300.0, 200.0, 1)
            })),
            vec![FollowOut::To(100.0)]
        );
    }

    #[test]
    fn a_picked_block_and_a_find_match_are_shown_once_and_stop_the_follow() {
        let mut follow = Follow::default();
        follow.step(FollowIn::Drawn(drawn(400.0, 500.0, 1)));
        let picked = Drawn {
            reveal: Some(BlockId(1)),
            reveal_span: Some((40.0, 400.0)),
            ..drawn(400.0, 500.0, 1)
        };
        assert_eq!(
            follow.step(FollowIn::Drawn(picked)),
            vec![FollowOut::Reveal(ScrollSpan {
                start: 40.0,
                length: VIEW
            })]
        );
        // Drawn again with the same pick: nothing more, and growing does not move it.
        assert_eq!(
            follow.step(FollowIn::Drawn(Drawn {
                total: 600.0,
                scroll: scroll(400.0, 600.0),
                ..picked
            })),
            vec![]
        );
        let found = Drawn {
            found: Some(7),
            found_top: Some(200.0),
            ..picked
        };
        assert_eq!(
            follow.step(FollowIn::Drawn(found)),
            vec![FollowOut::Reveal(ScrollSpan {
                start: 136.0,
                length: 96.0
            })]
        );
    }
}
