//! How much room the window has for the sidebar. Below [`NARROW_BELOW`] the sidebar starts
//! folded away, so the panes keep their width; the person can still show it at any width, and a
//! window that crosses the breakpoint starts over from what its width suggests.

use ds::base::vocab::Shown;
use ds::prelude::Px;

/// The window width under which the sidebar starts hidden, in logical pixels. The sidebar is
/// about 240 wide and a pane wants 60 columns (about 480 px) beside it, plus its insets.
pub const NARROW_BELOW: Px = Px(720.0);

/// The window's width, as far as the sidebar is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarRoom {
    /// Wide enough to show the sidebar beside the panes.
    Wide,
    /// Too narrow: the panes get the whole width until the person asks for the sidebar.
    Narrow,
}

impl SidebarRoom {
    /// The room a window of `width` has.
    pub fn of(width: Px) -> Self {
        match width.0 < NARROW_BELOW.0 {
            true => Self::Narrow,
            false => Self::Wide,
        }
    }

    /// Whether the sidebar starts shown at this width.
    pub fn shown(self) -> Shown {
        match self {
            Self::Wide => Shown::Visible,
            Self::Narrow => Shown::Hidden,
        }
    }
}

/// The sidebar after the show/hide action.
pub fn toggled(now: Shown) -> Shown {
    match now {
        Shown::Visible => Shown::Hidden,
        Shown::Hidden => Shown::Visible,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_starts_hidden_under_the_breakpoint_and_shown_from_it() {
        // (width, sidebar)
        let cases = [
            (1600.0, Shown::Visible),
            (720.0, Shown::Visible),
            (719.5, Shown::Hidden),
            (480.0, Shown::Hidden),
            (0.0, Shown::Hidden),
        ];
        for (width, want) in cases {
            assert_eq!(SidebarRoom::of(Px(width)).shown(), want, "{width}");
        }
    }

    #[test]
    fn the_action_flips_the_sidebar_at_every_width() {
        assert_eq!(toggled(Shown::Visible), Shown::Hidden);
        assert_eq!(toggled(Shown::Hidden), Shown::Visible);
    }
}
