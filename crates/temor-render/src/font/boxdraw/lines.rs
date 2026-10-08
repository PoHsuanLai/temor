//! Box-drawing lines (U+2500–257F except arcs and diagonals): light, heavy and double arms
//! from the cell's centre to its edges, so lines join across cells whatever the font.

use super::canvas::Canvas;

/// No arm, light, heavy, double.
const N: u8 = 0;
const L: u8 = 1;
const H: u8 = 2;
const D: u8 = 3;

/// The arms of a line character as `[up, right, down, left]`.
pub(crate) fn arms(c: char) -> Option<[u8; 4]> {
    let i = c as u32;
    let table: &[[u8; 4]] = match i {
        0x2500..=0x2503 => &[[N, L, N, L], [N, H, N, H], [L, N, L, N], [H, N, H, N]],
        0x250C..=0x254B => &CORNERS_AND_TEES,
        0x2550..=0x256C => &DOUBLES,
        0x2574..=0x257F => &HALVES,
        _ => return None,
    };
    let base = match i {
        0x2500..=0x2503 => 0x2500,
        0x250C..=0x254B => 0x250C,
        0x2550..=0x256C => 0x2550,
        _ => 0x2574,
    };
    table.get((i - base) as usize).copied()
}

/// U+250C–254B.
#[rustfmt::skip]
const CORNERS_AND_TEES: [[u8; 4]; 64] = [
    [N,L,L,N], [N,H,L,N], [N,L,H,N], [N,H,H,N], // ┌┍┎┏
    [N,N,L,L], [N,N,L,H], [N,N,H,L], [N,N,H,H], // ┐┑┒┓
    [L,L,N,N], [L,H,N,N], [H,L,N,N], [H,H,N,N], // └┕┖┗
    [L,N,N,L], [L,N,N,H], [H,N,N,L], [H,N,N,H], // ┘┙┚┛
    [L,L,L,N], [L,H,L,N], [H,L,L,N], [L,L,H,N], // ├┝┞┟
    [H,L,H,N], [H,H,L,N], [L,H,H,N], [H,H,H,N], // ┠┡┢┣
    [L,N,L,L], [L,N,L,H], [H,N,L,L], [L,N,H,L], // ┤┥┦┧
    [H,N,H,L], [H,N,L,H], [L,N,H,H], [H,N,H,H], // ┨┩┪┫
    [N,L,L,L], [N,L,L,H], [N,H,L,L], [N,H,L,H], // ┬┭┮┯
    [N,L,H,L], [N,L,H,H], [N,H,H,L], [N,H,H,H], // ┰┱┲┳
    [L,L,N,L], [L,L,N,H], [L,H,N,L], [L,H,N,H], // ┴┵┶┷
    [H,L,N,L], [H,L,N,H], [H,H,N,L], [H,H,N,H], // ┸┹┺┻
    [L,L,L,L], [L,L,L,H], [L,H,L,L], [L,H,L,H], // ┼┽┾┿
    [H,L,L,L], [L,L,H,L], [H,L,H,L], [H,L,L,H], // ╀╁╂╃
    [H,H,L,L], [L,L,H,H], [L,H,H,L], [H,H,L,H], // ╄╅╆╇
    [L,H,H,H], [H,L,H,H], [H,H,H,L], [H,H,H,H], // ╈╉╊╋
];

/// U+2550–256C.
#[rustfmt::skip]
const DOUBLES: [[u8; 4]; 29] = [
    [N,D,N,D], [D,N,D,N],                       // ═║
    [N,D,L,N], [N,L,D,N], [N,D,D,N],            // ╒╓╔
    [N,N,L,D], [N,N,D,L], [N,N,D,D],            // ╕╖╗
    [L,D,N,N], [D,L,N,N], [D,D,N,N],            // ╘╙╚
    [L,N,N,D], [D,N,N,L], [D,N,N,D],            // ╛╜╝
    [L,D,L,N], [D,L,D,N], [D,D,D,N],            // ╞╟╠
    [L,N,L,D], [D,N,D,L], [D,N,D,D],            // ╡╢╣
    [N,D,L,D], [N,L,D,L], [N,D,D,D],            // ╤╥╦
    [L,D,N,D], [D,L,N,L], [D,D,N,D],            // ╧╨╩
    [L,D,L,D], [D,L,D,L], [D,D,D,D],            // ╪╫╬
];

/// U+2574–257F.
#[rustfmt::skip]
const HALVES: [[u8; 4]; 12] = [
    [N,N,N,L], [L,N,N,N], [N,L,N,N], [N,N,L,N], // ╴╵╶╷
    [N,N,N,H], [H,N,N,N], [N,H,N,N], [N,N,H,N], // ╸╹╺╻
    [N,H,N,L], [L,N,H,N], [N,L,N,H], [H,N,L,N], // ╼╽╾╿
];

/// Draws the arms `[up, right, down, left]` with light strokes `t` pixels thick.
pub(crate) fn draw(canvas: &mut Canvas, arms: [u8; 4], t: u32) {
    let [up, right, down, left] = arms;
    vertical(canvas, [up, down], [left, right], t);
    // Horizontal arms are vertical arms of the transposed cell: left is "up", right "down",
    // and the perpendicular up/down become left/right.
    let mut turned = Canvas::new(canvas.h, canvas.w);
    vertical(&mut turned, [left, right], [up, down], t);
    canvas.merge(&turned.transposed());
}

/// Stroke thickness of a weight (a double's whole band: two strokes and the gap).
fn band(weight: u8, t: i32) -> i32 {
    match weight {
        L => t,
        H => t * 2,
        D => t * 3,
        _ => 0,
    }
}

/// Start of a band `k` wide centred in `len`.
fn centred(len: i32, k: i32) -> i32 {
    (len - k) / 2
}

/// The up and down arms, given the weights of the left and right arms they meet.
fn vertical(canvas: &mut Canvas, [up, down]: [u8; 2], [left, right]: [u8; 2], t: u32) {
    let (w, h, t) = (canvas.w as i32, canvas.h as i32, t.max(1) as i32);
    let across = left.max(right);
    let hb = band(across, t);
    let (hy0, hy1) = (centred(h, hb), centred(h, hb) + hb);
    for (weight, is_up) in [(up, true), (down, false)] {
        if weight == N {
            continue;
        }
        let k = band(weight, t);
        let x0 = centred(w, k);
        // Where an arm without a perpendicular stops: just past the centre.
        let mid = centred(h, k) + k;
        let mid_start = centred(h, k);
        let span = |x: i32, side: u8, canvas: &mut Canvas| {
            let (end, start) = if across == N {
                (mid, mid_start)
            } else if across == D && (weight != D || side == D) {
                // Meet the near stroke of a double: the inner corner.
                (hy0 + t, hy1 - t)
            } else {
                (hy1, hy0)
            };
            let width = if weight == D { t } else { k };
            if is_up {
                canvas.rect(x, 0, x + width, end);
            } else {
                canvas.rect(x, start, x + width, h);
            }
        };
        if weight == D {
            span(x0, left, canvas);
            span(x0 + 2 * t, right, canvas);
        } else {
            span(x0, N, canvas);
        }
    }
}

/// Dashed lines (U+2504–250B, 254C–254F): `(vertical, weight, dashes)`.
pub(crate) fn dashes(c: char) -> Option<(bool, u8, u32)> {
    Some(match c as u32 {
        0x2504 => (false, L, 3),
        0x2505 => (false, H, 3),
        0x2506 => (true, L, 3),
        0x2507 => (true, H, 3),
        0x2508 => (false, L, 4),
        0x2509 => (false, H, 4),
        0x250A => (true, L, 4),
        0x250B => (true, H, 4),
        0x254C => (false, L, 2),
        0x254D => (false, H, 2),
        0x254E => (true, L, 2),
        0x254F => (true, H, 2),
        _ => return None,
    })
}

/// Draws `n` dashes along a horizontal centre line (the caller transposes for vertical).
pub(crate) fn draw_dashes(canvas: &mut Canvas, weight: u8, n: u32, t: u32) {
    let (w, h) = (canvas.w as i32, canvas.h as i32);
    let k = band(weight, t.max(1) as i32);
    let y0 = centred(h, k);
    let n = n.max(1) as i32;
    for i in 0..n {
        let (a, b) = (i * w / n, (i + 1) * w / n);
        let gap = ((b - a) / 3).max(1);
        canvas.rect(a + gap / 2, y0, b - (gap - gap / 2), y0 + k);
    }
}
