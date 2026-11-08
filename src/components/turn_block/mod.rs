//! The turn block (product §4.2): one conversation turn as a rounded block of
//! sections.
//!
//! * [`TurnBlock`] is the widget: border, titles, the `▒` selection marker.
//! * [`derive_sections`] derives the sections of a turn and [`FoldState`] says
//!   which are folded.
//! * [`renderers`] has one renderer type per section kind.
//! * [`emphasize`] applies the selected / dimmed look.
//!
//! Everything is pure: heights and rows are computed from the turn, the fold
//! state and the width alone. The transcript (a later task) stacks blocks
//! into a virtualised view.

mod emphasis;
pub mod export;
pub mod plain;
pub mod renderers;
mod section;
mod title;
mod widget;

#[cfg(test)]
mod tests;

pub use emphasis::emphasize;
pub use section::{
    FoldState, Section, SectionContent, SectionId, SectionKey, SectionKind, derive_sections,
};
pub use title::TitleBar;
#[cfg(test)]
pub(crate) use widget::block_work;
pub use widget::{BlockLayout, TurnBlock};

#[cfg(test)]
mod durations;
#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod notice_snapshots;
#[cfg(test)]
mod snapshots;
#[cfg(test)]
mod subagent_title;
