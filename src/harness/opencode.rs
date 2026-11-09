//! The OpenCode adapter (product §2.2, §4.1).
//!
//! OpenCode keeps everything in one SQLite database. The adapter reads it
//! strictly read-only (see [`db`]): projects from `project`, top-level
//! sessions from `session_v2`, conversations from `session_message` (paged,
//! by `seq`), and subagent runs through the child sessions of `parent_id`.

pub mod assemble;
pub mod db;
mod details;
mod discovery;
#[cfg(test)]
pub(crate) mod fixtures;
mod follow;
mod load;
mod location;
mod subagent;
#[cfg(test)]
mod tests;
pub mod tools;
mod watch;

pub use assemble::Assembler;
pub use db::{Db, NEWEST_KNOWN_MIGRATION, OpenMode};
pub use discovery::{DEFAULT_RESUME_COMMAND, OpenCodeHarness};
pub use location::{candidate_db_paths, default_db_path};
