//! The macOS desktop: the login Keychain through porter's store and the Dock tile through AppKit.
//! There is no session bus here, so the rest of the services (single instance, the agent layer,
//! Tailscale, restore points) are the absent ones of `unsupported.rs`; single instance is a socket
//! (`instance.rs`).
//! TODO(platform:macos): the agent layer's transport once docket has one here.

mod instance;

pub use instance::SystemInstance;

use crate::platform::stored::{NAMESPACE, StoredSecrets};
use crate::secrets::{SecretFault, SecretVault};
use crate::{Badge, BadgeCount, DesktopEnv, DesktopError};
use dispatch2::DispatchQueue;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSApplication, NSRequestUserAttentionType};
use objc2_foundation::NSString;
use porter_secrets::{KeyringNamedSecrets, KeyringSecrets, Namespace};
use tokio::runtime::Handle;

/// A Space's secrets in the login Keychain, filed under temor's name.
#[derive(Debug)]
pub struct SystemSecrets {
    inner: StoredSecrets<KeyringNamedSecrets>,
}

impl SystemSecrets {
    /// The store, with calls waiting on `runtime`, which the caller keeps alive. The Keychain is
    /// first opened by the first ask, not here.
    pub fn new(runtime: Handle) -> Result<Self, SecretFault> {
        let namespace = Namespace::parse(NAMESPACE).map_err(|_| SecretFault::NoStore)?;
        Ok(Self {
            inner: StoredSecrets::over(KeyringSecrets::in_namespace(namespace), runtime),
        })
    }
}

impl SecretVault for SystemSecrets {
    fn get(&self, account: &str) -> Result<Option<String>, SecretFault> {
        self.inner.get(account)
    }

    fn put(&self, account: &str, secret: &str) -> Result<(), SecretFault> {
        self.inner.put(account, secret)
    }

    fn delete(&self, account: &str) -> Result<(), SecretFault> {
        self.inner.delete(account)
    }
}

/// The Dock tile's badge.
#[derive(Debug, Clone, Copy)]
pub struct SystemDock;

impl SystemDock {
    /// The Dock needs no connection; this always succeeds.
    pub async fn connect(_env: &DesktopEnv) -> Result<Self, DesktopError> {
        Ok(Self)
    }

    /// Show `badge` on temor's Dock icon: its count as the tile's label (none clears it), and
    /// one bounce when attention is needed. A progress bar has no place on a tile label, so it is
    /// not shown. AppKit is the main thread's, so the work is queued there; the window's loop
    /// (winit's) is that thread's run loop.
    pub async fn show(&self, badge: &Badge) -> Result<(), DesktopError> {
        let label = label_of(badge.count);
        let wants_attention = badge.wants_attention;
        DispatchQueue::main().exec_async(move || draw(label.as_deref(), wants_attention));
        Ok(())
    }
}

/// The text on the tile for `count`: a number, or nothing for none or zero.
fn label_of(count: Option<BadgeCount>) -> Option<String> {
    count
        .filter(|BadgeCount(n)| *n > 0)
        .map(|BadgeCount(n)| n.to_string())
}

fn draw(label: Option<&str>, attention: bool) {
    let Some(main) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(main);
    let text = label.map(NSString::from_str);
    app.dockTile().setBadgeLabel(text.as_deref());
    if attention {
        app.requestUserAttention(NSRequestUserAttentionType::InformationalRequest);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tile_shows_a_number_or_nothing() {
        assert_eq!(label_of(None), None);
        assert_eq!(label_of(Some(BadgeCount(0))), None);
        assert_eq!(label_of(Some(BadgeCount(12))).as_deref(), Some("12"));
    }
}
