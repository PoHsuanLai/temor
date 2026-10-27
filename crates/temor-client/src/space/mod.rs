//! What a Space's new shell is given from outside the process (PLAN §7): its dotenv file, its
//! secrets from the desktop's password store, and its own ssh-agent.

pub mod agent;
pub mod start;
pub mod vault;
