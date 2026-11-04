//! A value another thread or window publishes on a watch channel, as a signal the view reads:
//! its value now, and each new one as it is sent.

use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use tokio::sync::watch;

/// The channel `open` gives, as a signal; `otherwise` its value when there is no channel. The
/// signal changes only when the value does.
pub fn use_watch<T: Clone + PartialEq + 'static>(
    open: impl FnOnce() -> Option<watch::Receiver<T>>,
    otherwise: impl FnOnce() -> T,
) -> Signal<T> {
    let heard = use_hook(|| Rc::new(RefCell::new(open())));
    let mut value = use_signal({
        let heard = Rc::clone(&heard);
        move || match heard.borrow_mut().as_mut() {
            Some(heard) => heard.borrow_and_update().clone(),
            None => otherwise(),
        }
    });
    use_future(move || {
        let heard = heard.borrow_mut().take();
        async move {
            let Some(mut heard) = heard else { return };
            while heard.changed().await.is_ok() {
                let now = heard.borrow_and_update().clone();
                if *value.peek() != now {
                    value.set(now);
                }
            }
        }
    });
    value
}
