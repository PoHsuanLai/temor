//! Single instance over a Unix socket: the first launch listens on `instance.sock` in a private
//! folder, and a later launch asks it for a new window or tab and ends. There is no bus here.
//!
//! The folder is 0700 and the person's own, so only their programs can connect. The socket takes
//! `NewWindow` and `NewTab` only, which open a shell; `Run` starts any program and is never taken,
//! so `temor -e` is its own launch, as it is where no running temor can be reached.

use crate::{Claim, DesktopEnv, DesktopError, Handover, Primary, Request, RunRequest};
use std::ffi::OsString;
use std::io::ErrorKind;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;
use tokio::time::timeout;

const SOCKET: &str = "instance.sock";
/// What a launch waits for the running temor to take its request, or for a request to arrive.
const WAIT: Duration = Duration::from_secs(2);
/// The longest request line: a verb and a folder in hex.
const LINE_MAX: u64 = 8192;

/// Single instance on a socket in `env.instance_dir`.
#[derive(Debug, Clone)]
pub struct SystemInstance {
    dir: Option<PathBuf>,
}

impl SystemInstance {
    /// An instance claim in `env`.
    pub fn new(env: DesktopEnv) -> Self {
        Self {
            dir: env.instance_dir,
        }
    }

    /// `Run` is never handed over (see the module docs): there is no running temor to wait on,
    /// and this launch is the window itself.
    pub async fn run_and_wait(
        &self,
        _run: &RunRequest,
        _token: Option<&str>,
    ) -> Result<Option<i32>, DesktopError> {
        Ok(None)
    }

    /// Try to be the first temor. The first launch listens and gets [`Claim::Primary`]; a later
    /// one asks the first to do `request` and gets [`Claim::Forwarded`].
    pub async fn claim(&self, request: &Request) -> Result<Claim, DesktopError> {
        self.claim_with(request, None).await
    }

    /// As [`claim`](Self::claim); there is no activation token on macOS to hand on.
    pub async fn claim_with(
        &self,
        request: &Request,
        _token: Option<&str>,
    ) -> Result<Claim, DesktopError> {
        let (sender, requests) = unbounded_channel();
        let Some(dir) = &self.dir else {
            return Ok(Claim::Primary(Primary::new(requests, Box::new(()))));
        };
        let fail = |error: &dyn std::fmt::Display| DesktopError::bus("claim temor's socket", error);
        temor_shell::ensure_private_dir(dir).map_err(|e| fail(&e))?;
        let path = dir.join(SOCKET);
        if path.as_os_str().len() > temor_shell::SOCKET_PATH_MAX {
            return Err(fail(&"its path is too long for a socket"));
        }
        let forwardable = encode(request).is_some();
        // Two launches at once: the one that binds second finds the first listening, and asks it.
        for _ in 0..2 {
            if let Some(line) = encode(request)
                && forward(&path, &line).await.map_err(|e| fail(&e))?
            {
                return Ok(Claim::Forwarded);
            }
            match UnixListener::bind(&path) {
                Ok(listener) => {
                    let task = tokio::spawn(accept(listener, sender));
                    return Ok(Claim::Primary(Primary::new(
                        requests,
                        Box::new(Listening { path, task }),
                    )));
                }
                // Nobody answered, so the file is a dead temor's: clear it and bind again.
                Err(e) if e.kind() == ErrorKind::AddrInUse => {
                    if !forwardable || forward(&path, "ping\n").await.unwrap_or(false) {
                        break;
                    }
                    let _ = std::fs::remove_file(&path);
                }
                Err(e) => return Err(fail(&e)),
            }
        }
        // A `Run`, or a socket that cannot be had: this launch is a window of its own.
        Ok(Claim::Primary(Primary::new(requests, Box::new(()))))
    }
}

/// The first temor's hold on the socket: dropped, it stops listening and removes the file.
struct Listening {
    path: PathBuf,
    task: JoinHandle<()>,
}

impl Drop for Listening {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// The line that asks for `request`, or `None` for one the socket does not take.
fn encode(request: &Request) -> Option<String> {
    match request {
        Request::NewWindow => Some("window\n".to_owned()),
        Request::NewTab { cwd: None } => Some("tab\n".to_owned()),
        Request::NewTab { cwd: Some(cwd) } => {
            let hex: String = cwd
                .as_os_str()
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            Some(format!("tab {hex}\n"))
        }
        Request::Run(_) => None,
    }
}

/// The request a line asks for; `None` for anything else, which is refused.
fn decode(line: &str) -> Option<Request> {
    let mut words = line.split_whitespace();
    match (words.next()?, words.next(), words.next()) {
        ("window", None, None) => Some(Request::NewWindow),
        ("tab", None, None) => Some(Request::NewTab { cwd: None }),
        ("tab", Some(hex), None) => {
            let bytes: Option<Vec<u8>> = hex
                .as_bytes()
                .chunks(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
                .collect();
            let cwd = PathBuf::from(OsString::from_vec(bytes?));
            Some(Request::NewTab { cwd: Some(cwd) })
        }
        _ => None,
    }
}

/// Asks the temor listening at `path` for what `line` says. `Ok(false)` when none is there.
async fn forward(path: &Path, line: &str) -> std::io::Result<bool> {
    let mut stream = match timeout(WAIT, UnixStream::connect(path)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) if matches!(e.kind(), ErrorKind::NotFound | ErrorKind::ConnectionRefused) => {
            return Ok(false);
        }
        Ok(Err(e)) => return Err(e),
        Err(_) => return Ok(false),
    };
    stream.write_all(line.as_bytes()).await?;
    let mut reply = String::new();
    let heard = timeout(WAIT, BufReader::new(&mut stream).read_line(&mut reply)).await;
    Ok(matches!(heard, Ok(Ok(_))) && reply.trim() == "ok")
}

async fn accept(listener: UnixListener, sender: UnboundedSender<Handover>) {
    while let Ok((stream, _)) = listener.accept().await {
        tokio::spawn(answer(stream, sender.clone()));
    }
}

/// Reads one line, passes the request on, and says `ok`; anything else gets `no`.
async fn answer(mut stream: UnixStream, sender: UnboundedSender<Handover>) {
    let mut line = String::new();
    let read = timeout(
        WAIT,
        BufReader::new((&mut stream).take(LINE_MAX)).read_line(&mut line),
    )
    .await;
    let request = matches!(read, Ok(Ok(n)) if n > 0)
        .then(|| decode(&line))
        .flatten();
    let reply = match request {
        // "ping" is no request: it only asks whether anyone is listening.
        None if line.trim() == "ping" => "ok\n",
        None => "no\n",
        Some(request) => {
            let handover = Handover {
                request,
                token: None,
            };
            if sender.send(handover).is_ok() {
                "ok\n"
            } else {
                "no\n"
            }
        }
    };
    let _ = stream.write_all(reply.as_bytes()).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instance(dir: &Path) -> SystemInstance {
        SystemInstance::new(DesktopEnv::absent().with_instance_dir(dir))
    }

    #[test]
    fn a_request_survives_its_line() {
        for request in [
            Request::NewWindow,
            Request::NewTab { cwd: None },
            Request::NewTab {
                cwd: Some(PathBuf::from("/tmp/a b/é")),
            },
        ] {
            let line = encode(&request).expect("a line");
            assert_eq!(decode(&line), Some(request));
        }
        assert_eq!(decode("run ls\n"), None);
        assert_eq!(decode("tab zz\n"), None);
    }

    #[tokio::test]
    async fn a_second_launch_hands_its_request_to_the_first() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("i");
        let Claim::Primary(mut first) = instance(&dir)
            .claim(&Request::NewWindow)
            .await
            .expect("claim")
        else {
            panic!("the first launch is the primary");
        };
        let cwd = PathBuf::from("/tmp");
        let second = instance(&dir)
            .claim(&Request::NewTab {
                cwd: Some(cwd.clone()),
            })
            .await
            .expect("claim");
        assert!(matches!(second, Claim::Forwarded));
        assert_eq!(first.next().await, Some(Request::NewTab { cwd: Some(cwd) }));
    }

    #[tokio::test]
    async fn a_dead_temors_socket_is_taken_over() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("i");
        temor_shell::ensure_private_dir(&dir).expect("dir");
        // A crash leaves the file behind with nobody listening: std never unlinks a socket.
        drop(std::os::unix::net::UnixListener::bind(dir.join(SOCKET)).expect("bind"));
        let claim = instance(&dir)
            .claim(&Request::NewWindow)
            .await
            .expect("claim");
        assert!(matches!(claim, Claim::Primary(_)));
    }

    #[tokio::test]
    async fn a_run_is_never_handed_over() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("i");
        let _first = instance(&dir)
            .claim(&Request::NewWindow)
            .await
            .expect("claim");
        let run = Request::Run(RunRequest {
            argv: vec!["ls".into()],
            cwd: None,
            env: Vec::new(),
            ticket: None,
        });
        let second = instance(&dir).claim(&run).await.expect("claim");
        assert!(matches!(second, Claim::Primary(_)));
    }

    #[tokio::test]
    async fn the_socket_goes_with_the_first_launch() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("i");
        let first = instance(&dir)
            .claim(&Request::NewWindow)
            .await
            .expect("claim");
        assert!(dir.join(SOCKET).exists());
        drop(first);
        assert!(!dir.join(SOCKET).exists());
    }
}
