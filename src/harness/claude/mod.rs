//! The Claude Code adapter (SPEC §3.1, §5.1).

pub mod assemble;
mod discovery;
mod inject;
mod scan;
pub mod tools;
mod wire;

pub use assemble::Assembler;
pub use discovery::{
    BuildStats, ClaudeHarness, MAX_TITLE_CHARS, UNTITLED, default_data_dir, truncate_title,
};
pub use inject::NEWEST_KNOWN_VERSION;
pub(crate) use wire::extract_result_text;
