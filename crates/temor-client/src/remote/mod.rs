//! A host's helper (PLAN §9b, Tier B): the connection to it and the calls on it, putting it on
//! the host, noticing a stalled link, editing and sending files through it, and the typing held
//! while its link is down. What feeds a pane from it is `temor-panes`'.

pub mod adopt;
pub mod builds;
pub mod calls;
pub mod conn;
pub mod edit;
pub mod edit_stream;
pub mod grants;
pub mod hold;
pub mod install;
pub mod ping;
pub mod upload;
