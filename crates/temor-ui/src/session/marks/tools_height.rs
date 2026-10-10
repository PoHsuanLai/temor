//! The block toolbar's height as the window measured it (logical pixels), shared between the
//! toolbar's view, which measures, and the session, which counts the grid rows it covers.

use std::cell::Cell;
use std::rc::Rc;

use super::rows::TOOLS_HEIGHT_LOGICAL;

/// A measured height, shared by clones. Until the toolbar has been laid out it is the estimate
/// [`TOOLS_HEIGHT_LOGICAL`].
#[derive(Debug, Clone)]
pub struct ToolsHeight(Rc<Cell<f32>>);

impl Default for ToolsHeight {
    fn default() -> Self {
        ToolsHeight(Rc::new(Cell::new(TOOLS_HEIGHT_LOGICAL)))
    }
}

impl ToolsHeight {
    /// The height in logical pixels.
    pub fn get(&self) -> f32 {
        self.0.get()
    }

    /// Records a measured height. A height that is not a positive number is not a measurement.
    pub fn measured(&self, height: f32) {
        if height.is_finite() && height > 0.0 {
            self.0.set(height);
        }
    }
}

/// Two are equal when they are the same measurement, so a redraw that measured something new
/// does not make the toolbar's props differ.
impl PartialEq for ToolsHeight {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_real_measurement_replaces_the_estimate() {
        let height = ToolsHeight::default();
        assert_eq!(height.get(), TOOLS_HEIGHT_LOGICAL);
        for (what, value, want) in [
            ("zero is not a layout", 0.0, TOOLS_HEIGHT_LOGICAL),
            ("negative", -3.0, TOOLS_HEIGHT_LOGICAL),
            ("not a number", f32::NAN, TOOLS_HEIGHT_LOGICAL),
            ("a laid out toolbar", 31.5, 31.5),
        ] {
            height.measured(value);
            assert_eq!(height.get(), want, "{what}");
        }
        let shared = height.clone();
        shared.measured(40.0);
        assert_eq!(height.get(), 40.0, "clones share one measurement");
    }
}
