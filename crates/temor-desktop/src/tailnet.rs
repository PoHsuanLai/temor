//! The Tailscale computers list: the stream that says when to read it again.

use std::fmt;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A source of "the list changed" ticks. The Linux service implements it over the account
/// service's signal; keeping it a trait lets the portable build name no stream crate.
pub(crate) trait Ticks: Send {
    /// The next tick, `Ready(None)` once the source is gone.
    fn poll_tick(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<()>>;
}

/// One item each time the list changed, with nothing in it: read the list again. Where there is
/// no Tailscale the stream has already ended.
pub struct TailnetChanges {
    source: Option<Pin<Box<dyn Ticks>>>,
}

impl TailnetChanges {
    /// A stream with nothing to say.
    pub(crate) fn ended() -> Self {
        Self { source: None }
    }

    /// A stream over `source`.
    #[cfg_attr(not(feature = "quire-desktop"), allow(dead_code))]
    pub(crate) fn over(source: impl Ticks + 'static) -> Self {
        Self {
            source: Some(Box::pin(source)),
        }
    }

    /// The next change, or `None` when there will be no more.
    pub async fn next(&mut self) -> Option<()> {
        std::future::poll_fn(|cx| match self.source.as_mut() {
            Some(source) => source.as_mut().poll_tick(cx),
            None => Poll::Ready(None),
        })
        .await
    }
}

impl fmt::Debug for TailnetChanges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TailnetChanges").finish_non_exhaustive()
    }
}
