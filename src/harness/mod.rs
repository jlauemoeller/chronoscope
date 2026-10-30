//! Harness adapters: translate native session formats into the shared model.

pub mod claude;

use crate::model::{HarnessId, Project, ProjectId, SessionSummary};

/// Result type of harness operations.
pub use anyhow::Result;

/// A coding-agent harness whose sessions can be browsed (SPEC §1.1).
///
/// This is stage 1 (discovery). `load`, `subagent` and `resume_command` are
/// added with the session loader.
pub trait Harness {
    /// Which harness this is.
    fn id(&self) -> HarnessId;

    /// Whether the harness's data store was found.
    fn available(&self) -> bool;

    /// The projects, sorted by most recent activity, newest first.
    fn projects(&self) -> Result<Vec<Project>>;

    /// The sessions of `project`, sorted by start time, newest first.
    fn sessions(&self, project: &ProjectId) -> Result<Vec<SessionSummary>>;
}
