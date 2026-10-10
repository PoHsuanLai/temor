//! The calls on a helper's client that the window makes, in one place. The wire is being
//! revised (TODO(batch3-wire): `OpenSession` and `resize` gain the cell pixel size, and the
//! client gains `ping`); the rest of the window never names these types.

use temor_remote::{Attached, Client, Error};
use temor_wire::ids::SessionId;
use temor_wire::message::{EnvVar, ListeningPort, OpenSession, RemotePath};
use temor_wire::units::{Cols, Rows, Size};

/// The size of a terminal of `cols` by `rows` cells.
pub fn size_of(cols: u16, rows: u16) -> Size {
    Size {
        cols: Cols(cols),
        rows: Rows(rows),
    }
}

/// Opens a session running the account's login shell in `cwd`, with `env` as its whole
/// environment.
pub fn open(
    client: &Client,
    cwd: &str,
    env: &[(String, String)],
    size: Size,
) -> Result<SessionId, Error> {
    client.open_session(OpenSession {
        shell: None,
        cwd: RemotePath(cwd.to_owned()),
        env: env
            .iter()
            .map(|(name, value)| EnvVar {
                name: name.clone(),
                value: value.clone(),
            })
            .collect(),
        size,
    })
}

pub fn attach(client: &Client, session: SessionId) -> Result<Attached, Error> {
    client.attach(session)
}

pub fn resize(client: &Client, session: SessionId, size: Size) -> Result<(), Error> {
    client.resize(session, size)
}

pub fn input(client: &Client, session: SessionId, bytes: &[u8]) -> Result<(), Error> {
    client.send_input(session, bytes)
}

pub fn close(client: &Client, session: SessionId) -> Result<(), Error> {
    client.close_session(session)
}

pub fn ports(client: &Client) -> Result<Vec<ListeningPort>, Error> {
    client.list_ports()
}
