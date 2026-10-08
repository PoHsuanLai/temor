//! Reading a `Terminal`'s state: title, working directory, size, scrollback position, keyboard
//! flags and hyperlinks.

use temor_vt_sys as sys;

use super::convert::{check, string};
use super::terminal::Terminal;
use crate::{
    ColorScheme, GridPoint, GridSize, KittyKeyFlags, PointSpace, RowCount, Scrollbar, VtError,
};

impl Terminal {
    /// The title the program set (OSC 0/2); empty when none.
    pub fn title(&self) -> String {
        self.string(sys::TerminalData::TITLE)
    }

    /// The working directory the program reported (OSC 7 URL or a bare path); empty when none.
    pub fn pwd(&self) -> String {
        self.string(sys::TerminalData::PWD)
    }

    fn string(&self, data: sys::TerminalData::Type) -> String {
        let mut s = sys::String {
            ptr: std::ptr::null(),
            len: 0,
        };
        // SAFETY: TITLE and PWD write a borrowed `GhosttyString`, valid until the next mutating
        // call; it is copied before `self` can be mutated.
        unsafe {
            sys::ghostty_terminal_get(self.raw(), data, (&raw mut s).cast());
            string(s)
        }
    }

    /// The grid size in cells.
    pub fn size(&self) -> GridSize {
        // SAFETY: the terminal is live.
        unsafe { raw_size(self.raw()) }
    }

    /// Where the viewport is over the scrollback.
    pub fn scrollbar(&self) -> Result<Scrollbar, VtError> {
        let mut bar = sys::TerminalScrollbar::default();
        // SAFETY: SCROLLBAR writes a `GhosttyTerminalScrollbar`.
        check(
            unsafe {
                sys::ghostty_terminal_get(
                    self.raw(),
                    sys::TerminalData::SCROLLBAR,
                    (&raw mut bar).cast(),
                )
            },
            "scrollbar",
        )?;
        Ok(Scrollbar {
            total: RowCount(bar.total),
            offset: RowCount(bar.offset),
            len: RowCount(bar.len),
        })
    }

    /// The kitty keyboard flags the program pushed.
    pub fn kitty_keyboard_flags(&self) -> KittyKeyFlags {
        let mut flags: u8 = 0;
        // SAFETY: KITTY_KEYBOARD_FLAGS writes a `uint8_t`.
        unsafe {
            sys::ghostty_terminal_get(
                self.raw(),
                sys::TerminalData::KITTY_KEYBOARD_FLAGS,
                (&raw mut flags).cast(),
            )
        };
        KittyKeyFlags::from_bits(flags)
    }

    /// The scheme reported to programs that ask (CSI ? 996 n).
    pub fn set_color_scheme(&mut self, scheme: ColorScheme) {
        self.callbacks().color_scheme.set(scheme);
    }

    /// The OSC 8 hyperlink address of the cell at `at`, if it has one.
    pub fn hyperlink_at(&self, at: GridPoint) -> Result<Option<String>, VtError> {
        let gref = self.grid_ref(at)?;
        let mut len = 0usize;
        // SAFETY: a null buffer asks for the length; `gref` was just made from this terminal.
        let code = unsafe {
            sys::ghostty_grid_ref_hyperlink_uri(&gref, std::ptr::null_mut(), 0, &mut len)
        };
        if code == sys::Result::SUCCESS || len == 0 {
            return Ok(None);
        }
        let mut buf = vec![0u8; len];
        // SAFETY: `buf` holds `len` writable bytes.
        check(
            unsafe {
                sys::ghostty_grid_ref_hyperlink_uri(&gref, buf.as_mut_ptr(), buf.len(), &mut len)
            },
            "hyperlink",
        )?;
        buf.truncate(len);
        Ok(Some(String::from_utf8_lossy(&buf).into_owned()))
    }

    /// A grid reference for `at`, valid until the terminal is next mutated.
    pub(super) fn grid_ref(&self, at: GridPoint) -> Result<sys::GridRef, VtError> {
        let tag = match at.space {
            PointSpace::Active => sys::PointTag::ACTIVE,
            PointSpace::Viewport => sys::PointTag::VIEWPORT,
            PointSpace::Screen => sys::PointTag::SCREEN,
        };
        let mut value = sys::PointValue::default();
        value.coordinate = sys::PointCoordinate {
            x: at.col.0,
            y: at.row,
        };
        let mut gref = temor_vt_sys::sized!(sys::GridRef);
        // SAFETY: the terminal is live; `gref` is a valid out-pointer.
        check(
            unsafe {
                sys::ghostty_terminal_grid_ref(self.raw(), sys::Point { tag, value }, &mut gref)
            },
            "point outside the grid",
        )?;
        Ok(gref)
    }
}

/// # Safety
/// `raw` must be a live terminal.
pub(super) unsafe fn raw_size(raw: sys::Terminal) -> GridSize {
    let (mut cols, mut rows) = (0u16, 0u16);
    // SAFETY: COLS and ROWS write a `uint16_t`; `raw` is live per the contract.
    unsafe {
        sys::ghostty_terminal_get(raw, sys::TerminalData::COLS, (&raw mut cols).cast());
        sys::ghostty_terminal_get(raw, sys::TerminalData::ROWS, (&raw mut rows).cast());
    }
    GridSize::new(cols, rows)
}
