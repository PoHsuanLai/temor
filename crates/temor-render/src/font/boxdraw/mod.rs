//! Box drawing, block elements and Powerline arrows, drawn to fill the cell exactly (as Ghostty
//! and kitty do) instead of taken from the font, so they join across cells and rows.

mod canvas;
mod lines;
mod shapes;
#[cfg(test)]
mod tests;

pub(crate) use canvas::Canvas;

/// Whether `c` is drawn here rather than from a font.
pub(crate) fn is_procedural(c: char) -> bool {
    matches!(c as u32, 0x2500..=0x259F | 0xE0B0..=0xE0B3)
}

/// The coverage mask of `c` for a `w` x `h` cell with light strokes `t` thick, or `None` when
/// `c` is not drawn here.
pub(crate) fn draw(c: char, w: u32, h: u32, t: u32) -> Option<Vec<u8>> {
    if !is_procedural(c) || w == 0 || h == 0 {
        return None;
    }
    let mut canvas = Canvas::new(w, h);
    if let Some(arms) = lines::arms(c) {
        lines::draw(&mut canvas, arms, t);
    } else if let Some((vertical, weight, n)) = lines::dashes(c) {
        if vertical {
            let mut turned = Canvas::new(h, w);
            lines::draw_dashes(&mut turned, weight, n, t);
            canvas = turned.transposed();
        } else {
            lines::draw_dashes(&mut canvas, weight, n, t);
        }
    } else if !(shapes::block(&mut canvas, c)
        || shapes::curve(&mut canvas, c, t)
        || shapes::powerline(&mut canvas, c, t))
    {
        return None;
    }
    Some(canvas.data)
}
