//! How narrow a pane may get. A terminal under about 60 columns wraps most output into noise, so
//! a split that would leave a pane below that is refused.

use super::model::Direction;

/// A width in terminal columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Columns(pub u32);

/// The least width a pane keeps after a split.
pub const LEAST_COLUMNS: Columns = Columns(60);

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
    fn a_side_by_side_split_keeps_sixty_columns_in_each_half() {
        // (name, width, towards, allowed)
        let cases = [
            ("wide enough for two", 120, Direction::Right, true),
            ("one column short", 119, Direction::Left, false),
            ("a narrow pane", 80, Direction::Right, false),
            ("stacking keeps the width", 80, Direction::Down, true),
            ("stacking in a narrow window", 30, Direction::Up, true),
        ];
        for (name, width, towards, allowed) in cases {
            assert_eq!(split_fits(Columns(width), towards), allowed, "{name}");
        }
    }
}
