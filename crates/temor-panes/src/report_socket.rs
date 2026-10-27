//! The pane sockets (PLAN §8.1, source 2): each pane gets `<runtime>/<pane>.sock` (0600, in a
//! 0700 folder), named to its program in `TEMOR_SOCKET`. One thread at the edge accepts the
//! connections of every pane and sends what they report to the window's event channel, tagged with
//! the pane. temor-pane's socket is a library the caller pumps; this is the caller.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use temor_core::vocab::ids::PaneId;
use temor_pane::PaneEvent;
use temor_pane::report::{PaneKey, PaneSocket, SocketLimits};
use tokio::sync::mpsc::UnboundedSender;

/// How long the thread waits for a request before it looks at the sockets again. A report is
/// heard within this.
const LOOK_EVERY: Duration = Duration::from_millis(100);

enum Request {
    Add(PaneId, PaneSocket),
    /// Drop the pane's socket; the sender, when there is one, is told once it is gone.
    Remove(PaneId, Option<Sender<()>>),
}

/// The pane sockets of one window. Dropping it stops the thread and removes the socket files.
#[derive(Debug)]
pub struct SocketHost {
    dir: PathBuf,
    requests: Sender<Request>,
}

impl SocketHost {
    /// A host that binds in `dir` (temor's runtime folder) and sends reports to `events`.
    pub fn start(
        dir: PathBuf,
        events: UnboundedSender<(PaneId, PaneEvent)>,
    ) -> std::io::Result<SocketHost> {
        let (requests, inbox) = channel();
        std::thread::Builder::new()
            .name("temor-pane-sockets".to_owned())
            .spawn(move || serve(inbox, events))?;
        Ok(SocketHost { dir, requests })
    }

    /// Listens for `pane` and returns the socket's path for its `TEMOR_SOCKET`. A socket the pane
    /// had before (a restarted shell) is replaced.
    pub fn open(&self, pane: PaneId) -> Result<PathBuf, temor_pane::PaneError> {
        let key = PaneKey::new(&pane.0.to_string())?;
        // An older socket of this pane is gone first: its drop removes the file by name, which
        // must not be the new socket's.
        let (gone, told) = channel();
        if self
            .requests
            .send(Request::Remove(pane, Some(gone)))
            .is_ok()
        {
            let _ = told.recv_timeout(Duration::from_secs(1));
        }
        let socket = PaneSocket::bind(&self.dir, key, SocketLimits::default())?;
        let path = socket.path().to_path_buf();
        // The thread is gone only when the window is; the pane then has no socket to report to.
        let _ = self.requests.send(Request::Add(pane, socket));
        Ok(path)
    }

    /// Stops listening for `pane` and removes its socket file.
    pub fn close(&self, pane: PaneId) {
        let _ = self.requests.send(Request::Remove(pane, None));
    }

    /// The folder the sockets are in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

fn serve(inbox: Receiver<Request>, events: UnboundedSender<(PaneId, PaneEvent)>) {
    let mut sockets: BTreeMap<PaneId, PaneSocket> = BTreeMap::new();
    loop {
        match inbox.recv_timeout(LOOK_EVERY) {
            Ok(request) => apply(&mut sockets, request),
            Err(RecvTimeoutError::Timeout) => {}
            // The window is gone: the sockets drop here and remove their files.
            Err(RecvTimeoutError::Disconnected) => return,
        }
        while let Ok(request) = inbox.try_recv() {
            apply(&mut sockets, request);
        }
        for (pane, socket) in &mut sockets {
            for line in socket.pump() {
                if events.send((*pane, PaneEvent::Reported(line))).is_err() {
                    return;
                }
            }
        }
    }
}

fn apply(sockets: &mut BTreeMap<PaneId, PaneSocket>, request: Request) {
    match request {
        Request::Add(pane, socket) => {
            sockets.insert(pane, socket);
        }
        Request::Remove(pane, told) => {
            sockets.remove(&pane);
            if let Some(told) = told {
                let _ = told.send(());
            }
        }
    }
}
