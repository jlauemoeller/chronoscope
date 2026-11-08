//! The resume command of a Claude session (product §5.7).

pub(super) use crate::harness::resume::build;

/// The default command template: `{id}` stands for the session id.
pub const DEFAULT_RESUME_COMMAND: &str = "claude --resume {id}";
