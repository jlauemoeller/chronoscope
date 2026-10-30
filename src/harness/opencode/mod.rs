//! The OpenCode adapter (SPEC §3.2, §5.1).
//!
//! OpenCode keeps everything in one SQLite database. The adapter reads it
//! strictly read-only (see [`db`]): projects from `project`, top-level
//! sessions from `session_v2`, conversations from `session_message` (paged,
//! by `seq`), and subagent runs through the child sessions of `parent_id`.

pub mod assemble;
pub mod db;
mod discovery;
mod follow;
mod load;
mod location;
mod subagent;
#[cfg(test)]
pub(crate) mod testing;
#[cfg(test)]
mod tests;
pub mod tools;
mod watch;

pub use assemble::Assembler;
pub use db::{Db, NEWEST_KNOWN_MIGRATION, OpenMode};
pub use discovery::{DEFAULT_RESUME_COMMAND, OpenCodeHarness};
pub use location::{candidate_db_paths, default_db_path};
