//! How narrow a pane may get. A terminal under about 40 columns wraps most output into noise, so
//! a split that would leave a pane below that is refused. (60 refused a side-by-side split in any
//! window under about 1270 px wide with the sidebar shown, which is most laptop windows.)

use super::model::Direction;

/// A width in terminal columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Columns(pub u32);

/// The least width a pane keeps after a split.
pub const LEAST_COLUMNS: Columns = Columns(40);

/// Whether a pane `width` columns wide may be split `towards`. Splitting left or right halves the
/// width, so both halves must keep [`LEAST_COLUMNS`]; splitting up or down keeps the width, so it
/// is always allowed (a window already narrower than the least is not made unsplittable).
pub fn split_fits(width: Columns, towards: Direction) -> bool {
    match towards {
        Direction::Left | Direction::Right => width.0 / 2 >= LEAST_COLUMNS.0,
        Direction::Up | Direction::Down => true,
    }
}

#[cfg(test)]
mod tests {
    use super::{Columns, split_fits};
    use crate::split::Direction;

    #[test]
    fn a_side_by_side_split_keeps_forty_columns_in_each_half() {
        // (name, width, towards, allowed)
        let cases = [
            ("wide enough for two", 80, Direction::Right, true),
            ("one column short", 79, Direction::Left, false),
            ("a narrow pane", 60, Direction::Right, false),
            ("stacking keeps the width", 80, Direction::Down, true),
            ("stacking in a narrow window", 30, Direction::Up, true),
        ];
        for (name, width, towards, allowed) in cases {
            assert_eq!(split_fits(Columns(width), towards), allowed, "{name}");
        }
    }
}
