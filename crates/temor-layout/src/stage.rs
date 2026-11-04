//! A row's stage and console (PLAN §7, v14): the page a command asked for, shown beside the
//! terminal it came from, and the way back and forward through the pages the row has shown, as a
//! browser tab's. Pure: the window keeps one of each per row and says what happened.

/// What a row shows of its stage and its console.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Layout {
    /// The page and the terminal side by side.
    #[default]
    Both,
    /// The page alone.
    Stage,
    /// The terminal alone; the page waits behind it.
    Console,
}

impl Layout {
    /// Whether the page is drawn.
    pub fn shows_stage(self) -> bool {
        matches!(self, Layout::Both | Layout::Stage)
    }

    /// Whether the terminal is drawn.
    pub fn shows_console(self) -> bool {
        matches!(self, Layout::Both | Layout::Console)
    }

    /// The console toggled: shown beside the page, or hidden behind it.
    pub fn toggle_console(self) -> Layout {
        match self {
            Layout::Both | Layout::Console => Layout::Stage,
            Layout::Stage => Layout::Both,
        }
    }
}

/// The pages a row has shown: the one on its stage, the ones Back returns to (newest last), and
/// the ones Forward goes on to (next last).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pages<T> {
    back: Vec<T>,
    current: Option<T>,
    forward: Vec<T>,
}

impl<T> Default for Pages<T> {
    fn default() -> Self {
        Pages {
            back: Vec::new(),
            current: None,
            forward: Vec::new(),
        }
    }
}

/// How many pages Back remembers; the oldest go first.
pub const REMEMBERED: usize = 50;

impl<T: Clone + PartialEq> Pages<T> {
    /// The page on the stage.
    pub fn current(&self) -> Option<&T> {
        self.current.as_ref()
    }

    /// `page` was opened: the one before it is where Back goes, and Forward has nowhere to go.
    /// Opening the page already shown changes nothing.
    pub fn visit(&mut self, page: T) {
        if self.current.as_ref() == Some(&page) {
            return;
        }
        if let Some(was) = self.current.replace(page) {
            self.back.push(was);
            if self.back.len() > REMEMBERED {
                self.back.remove(0);
            }
        }
        self.forward.clear();
    }

    /// The page was closed: the stage is empty, and Back opens it again.
    pub fn close(&mut self) {
        if let Some(was) = self.current.take() {
            self.back.push(was);
        }
        self.forward.clear();
    }

    /// Whether Back has somewhere to go: a page before this one, or the console behind the first.
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty() || self.current.is_some()
    }

    /// Whether Forward has a page to open.
    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// Back: the page before this one, now on the stage; `None` when there is none, and from the
    /// first page the stage empties (the console it was opened from), with Forward to reopen it.
    pub fn back(&mut self) -> Option<&T> {
        let page = self.back.pop();
        if let Some(was) = self.current.take() {
            self.forward.push(was);
        }
        self.current = page;
        self.current.as_ref()
    }

    /// Forward: the page Back left, now on the stage; `None` when there is none.
    pub fn forward(&mut self) -> Option<&T> {
        let page = self.forward.pop()?;
        if let Some(was) = self.current.replace(page) {
            self.back.push(was);
        }
        self.current.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn back_and_forward_walk_the_pages_as_a_browser_does() {
        let mut pages = Pages::default();
        assert!(!pages.can_go_back());
        pages.visit("a");
        pages.visit("b");
        pages.visit("c");
        assert_eq!(pages.back(), Some(&"b"));
        assert_eq!(pages.back(), Some(&"a"));
        assert!(pages.can_go_back(), "the console is before the first");
        assert_eq!(pages.back(), None, "the console");
        assert_eq!(pages.current(), None);
        assert!(!pages.can_go_back());
        assert_eq!(pages.forward(), Some(&"a"));
        assert_eq!(pages.forward(), Some(&"b"));
        // A page opened from here drops what Forward had.
        pages.visit("d");
        assert!(!pages.can_go_forward());
        assert_eq!(pages.back(), Some(&"b"));
    }

    #[test]
    fn the_same_page_again_is_not_a_step_and_a_closed_page_is_where_back_goes() {
        let mut pages = Pages::default();
        pages.visit("a");
        pages.visit("a");
        assert_eq!(pages.back(), None, "only the console before it");
        assert_eq!(pages.forward(), Some(&"a"));
        pages.close();
        assert_eq!(pages.current(), None);
        assert_eq!(pages.back(), Some(&"a"));
        assert_eq!(pages.current(), Some(&"a"));
    }

    #[test]
    fn back_remembers_only_so_many() {
        let mut pages = Pages::default();
        for n in 0..REMEMBERED + 10 {
            pages.visit(n);
        }
        let mut steps = 0;
        while pages.back().is_some() {
            steps += 1;
        }
        assert_eq!(steps, REMEMBERED);
        assert_eq!(pages.forward(), Some(&9), "the oldest remembered");
    }

    #[test]
    fn the_console_toggles_beside_the_page() {
        assert_eq!(Layout::Both.toggle_console(), Layout::Stage);
        assert_eq!(Layout::Stage.toggle_console(), Layout::Both);
        assert_eq!(Layout::Console.toggle_console(), Layout::Stage);
        assert!(Layout::Console.shows_console() && !Layout::Console.shows_stage());
    }

    proptest! {
        /// Back then Forward comes back to the same page, whatever was opened before.
        #[test]
        fn back_then_forward_is_where_it_started(opened in prop::collection::vec(0u8..6, 1..20)) {
            let mut pages = Pages::default();
            for page in opened {
                pages.visit(page);
            }
            let here = pages.current().copied();
            if pages.back().is_some() {
                prop_assert_eq!(pages.forward().copied(), here);
            }
        }
    }
}
