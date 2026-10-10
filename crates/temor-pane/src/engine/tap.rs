//! A listener on a program's raw output, before the terminal parses it. The window uses it to
//! read what a host prints ahead of its shell (a browser check's link), which the screen's text
//! may wrap or redraw.

use std::fmt::Debug;

/// Hears every chunk the program wrote, in order, on the pane's I/O thread. It must not block.
pub trait ByteTap: Send + Sync + Debug {
    /// One chunk of the program's output, exactly as read.
    fn bytes(&self, data: &[u8]);
}
