//! The terminal's default colours and its selection, set by the host.

use temor_vt_sys as sys;

use super::convert::check;
use super::terminal::Terminal;
use crate::{Rgb, SelectionShape, TextSelection, VtError};

fn c(rgb: Rgb) -> sys::ColorRgb {
    sys::ColorRgb {
        r: rgb.r,
        g: rgb.g,
        b: rgb.b,
    }
}

impl Terminal {
    /// The default foreground (cells without a colour). A program can still override it
    /// (OSC 10) until it resets it (OSC 110). `None` restores libghostty's own default.
    pub fn set_default_foreground(&mut self, color: Option<Rgb>) -> Result<(), VtError> {
        self.set_color(sys::TerminalOption::COLOR_FOREGROUND, color)
    }

    /// The default background; OSC 11 overrides it as for the foreground.
    pub fn set_default_background(&mut self, color: Option<Rgb>) -> Result<(), VtError> {
        self.set_color(sys::TerminalOption::COLOR_BACKGROUND, color)
    }

    /// The default cursor colour; OSC 12 overrides it.
    pub fn set_default_cursor(&mut self, color: Option<Rgb>) -> Result<(), VtError> {
        self.set_color(sys::TerminalOption::COLOR_CURSOR, color)
    }

    /// The default 256-colour palette; OSC 4 overrides single entries. `None` restores
    /// libghostty's built-in palette.
    pub fn set_default_palette(&mut self, palette: Option<&[Rgb; 256]>) -> Result<(), VtError> {
        let raw: Option<[sys::ColorRgb; 256]> = palette.map(|p| p.map(c));
        let ptr = raw
            .as_ref()
            .map_or(std::ptr::null(), |p| p.as_ptr().cast::<std::ffi::c_void>());
        // SAFETY: COLOR_PALETTE takes a pointer to 256 `GhosttyColorRgb` (or null), read during
        // the call; `raw` outlives it.
        check(
            unsafe {
                sys::ghostty_terminal_set(self.raw(), sys::TerminalOption::COLOR_PALETTE, ptr)
            },
            "palette",
        )
    }

    fn set_color(
        &mut self,
        option: sys::TerminalOption::Type,
        color: Option<Rgb>,
    ) -> Result<(), VtError> {
        let raw = color.map(c);
        let ptr = raw.as_ref().map_or(std::ptr::null(), |v| {
            std::ptr::from_ref(v).cast::<std::ffi::c_void>()
        });
        // SAFETY: the colour options take a `GhosttyColorRgb*` (or null to unset), read during
        // the call; `raw` outlives it.
        check(
            unsafe { sys::ghostty_terminal_set(self.raw(), option, ptr) },
            "colour",
        )
    }

    /// Sets the active screen's selection (cells the renderer then draws selected), or clears
    /// it with `None`. Fails when an end is outside the grid.
    pub fn set_selection(&mut self, selection: Option<TextSelection>) -> Result<(), VtError> {
        let Some(s) = selection else {
            // SAFETY: a null value clears the selection.
            return check(
                unsafe {
                    sys::ghostty_terminal_set(
                        self.raw(),
                        sys::TerminalOption::SELECTION,
                        std::ptr::null(),
                    )
                },
                "selection",
            );
        };
        let mut raw = temor_vt_sys::sized!(sys::Selection);
        raw.start = self.grid_ref(s.start)?;
        raw.end = self.grid_ref(s.end)?;
        raw.rectangle = s.shape == SelectionShape::Block;
        // SAFETY: SELECTION takes a sized `GhosttySelection*` whose grid refs were just made from
        // this terminal and are copied before the call returns.
        check(
            unsafe {
                sys::ghostty_terminal_set(
                    self.raw(),
                    sys::TerminalOption::SELECTION,
                    (&raw const raw).cast(),
                )
            },
            "selection",
        )
    }
}
