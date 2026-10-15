//! SSH in the hub, with stand-in `ssh` and `scp` programs in a scratch folder: the questions
//! before a first connection, the shared connection, the browser check and the remote files.
//! Nothing here reaches a network, the real `~/.ssh` or the desktop.

mod connect;
mod edit;
mod hosts;
mod live;
mod local_tab;
mod retry;
mod support;
mod upload;
