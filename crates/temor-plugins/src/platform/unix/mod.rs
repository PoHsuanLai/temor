//! The Unix half: bayonet's runner, temor's protocol on it, and one method for each request.

mod launcher;
mod requests;
mod wire;

pub use launcher::Launcher;
