//! The Claude Code adapter (SPEC §3.1, §5.1).

pub mod assemble;
mod discovery;
mod inject;
mod load;
mod resume;
mod scan;
mod subagent;
pub mod tools;
mod watch;
mod wire;

pub use crate::harness::shared::{MAX_TITLE_CHARS, UNTITLED, truncate_title};
pub use assemble::Assembler;
pub use discovery::{BuildStats, ClaudeHarness, default_data_dir};
pub use inject::NEWEST_KNOWN_VERSION;
pub use resume::DEFAULT_RESUME_COMMAND;
pub use subagent::SubagentInfo;
pub(crate) use wire::extract_result_text;
