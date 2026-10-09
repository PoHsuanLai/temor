//! Block elements (U+2580–259F), rounded corners and diagonals (U+256D–2573), and the
//! Powerline arrows (U+E0B0–E0B3).

use super::canvas::Canvas;

/// Draws a block element. `false` when `c` is not one.
pub(crate) fn block(canvas: &mut Canvas, c: char) -> bool {
    let (w, h) = (canvas.w as i32, canvas.h as i32);
    // Eighths, rounded so neighbouring cells tile without seams.
    let ex = |n: i32| (w * n + 4) / 8;
    let ey = |n: i32| (h * n + 4) / 8;
    let (hx, hy) = (ex(4), ey(4));
    match c as u32 {
        0x2580 => canvas.rect(0, 0, w, hy),
        n @ 0x2581..=0x2588 => canvas.rect(0, h - ey(n as i32 - 0x2580), w, h),
        n @ 0x2589..=0x258F => canvas.rect(0, 0, ex(0x2590 - n as i32), h),
        0x2590 => canvas.rect(hx, 0, w, h),
        n @ 0x2591..=0x2593 => canvas.rect_alpha(0, 0, w, h, (64 * (n - 0x2590)) as u8),
        0x2594 => canvas.rect(0, 0, w, ey(1)),
        0x2595 => canvas.rect(w - ex(1), 0, w, h),
        n @ 0x2596..=0x259F => {
            // Quadrants as [upper-left, upper-right, lower-left, lower-right].
            let q: [u8; 4] = match n {
                0x2596 => [0, 0, 1, 0],
                0x2597 => [0, 0, 0, 1],
                0x2598 => [1, 0, 0, 0],
                0x2599 => [1, 0, 1, 1],
                0x259A => [1, 0, 0, 1],
                0x259B => [1, 1, 1, 0],
                0x259C => [1, 1, 0, 1],
                0x259D => [0, 1, 0, 0],
                0x259E => [0, 1, 1, 0],
                _ => [0, 1, 1, 1],
            };
            let quads = [
                (0, 0, hx, hy),
                (hx, 0, w, hy),
                (0, hy, hx, h),
                (hx, hy, w, h),
            ];
            for (on, (x0, y0, x1, y1)) in q.into_iter().zip(quads) {
                if on == 1 {
                    canvas.rect(x0, y0, x1, y1);
                }
            }
        }
        _ => return false,
    }
    true
}

/// Draws a rounded corner or a diagonal with strokes `t` thick. `false` when `c` is neither.
pub(crate) fn curve(canvas: &mut Canvas, c: char, t: u32) -> bool {
    let (w, h) = (canvas.w as f32, canvas.h as f32);
    let t = t.max(1) as f32;
    // The line centres the straight arms use: a band `t` wide centred in the cell.
    let cx = ((canvas.w as i32 - t as i32) / 2) as f32 + t / 2.0;
    let cy = ((canvas.h as i32 - t as i32) / 2) as f32 + t / 2.0;
    let half = t / 2.0;
    match c as u32 {
        n @ 0x256D..=0x2570 => {
            // ╭╮╯╰: a quarter circle joining the two arms' centre lines.
            let r = (w.min(h) / 2.0).min(cx).min(w - cx);
            let (sx, sy) = match n {
                0x256D => (1.0, 1.0),
                0x256E => (-1.0, 1.0),
                0x256F => (-1.0, -1.0),
                _ => (1.0, -1.0),
            };
            let (ox, oy) = (cx + sx * r, cy + sy * r);
            canvas.shape(|x, y| {
                let (dx, dy) = (x - ox, y - oy);
                // Arc in the quadrant facing the centre; straight arms beyond it.
                let in_corner = dx * sx <= 0.0 && dy * sy <= 0.0;
                let arc = in_corner && ((dx * dx + dy * dy).sqrt() - r).abs() <= half;
                let horizontal = dx * sx > 0.0 && (y - cy).abs() <= half;
                let vertical = dy * sy > 0.0 && (x - cx).abs() <= half;
                arc || horizontal || vertical
            });
        }
        n @ 0x2571..=0x2573 => {
            // ╱╲╳ from corner to corner, so they join across cells.
            let len = (w * w + h * h).sqrt();
            let rising = move |x: f32, y: f32| ((x * h + y * w - w * h) / len).abs() <= half;
            let falling = move |x: f32, y: f32| ((x * h - y * w) / len).abs() <= half;
            match n {
                0x2571 => canvas.shape(rising),
                0x2572 => canvas.shape(falling),
                _ => canvas.shape(|x, y| rising(x, y) || falling(x, y)),
            }
        }
        _ => return false,
    }
    true
}

/// Draws a Powerline arrow. `false` when `c` is not one.
pub(crate) fn powerline(canvas: &mut Canvas, c: char, t: u32) -> bool {
    let (w, h) = (canvas.w as f32, canvas.h as f32);
    let half = t.max(1) as f32 / 2.0;
    // The edge from a top corner to the middle of the far side, as a distance function.
    let len = (w * w + h * h / 4.0).sqrt();
    let edge = move |x: f32, y: f32| (x * h / 2.0 - y.min(h - y) * w) / len;
    match c as u32 {
        0xE0B0 => canvas.shape(|x, y| edge(x, y) <= 0.0),
        0xE0B1 => canvas.shape(|x, y| edge(x, y).abs() <= half),
        0xE0B2 => canvas.shape(|x, y| edge(w - x, y) <= 0.0),
        0xE0B3 => canvas.shape(|x, y| edge(w - x, y).abs() <= half),
        _ => return false,
    }
    true
}
