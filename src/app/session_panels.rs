//! The Session Time and Session Tokens panels' values (product §3.3): which
//! session they show, and pushing its stats into them.
//!
//! One function works out the session ([`App::panels_session`]) and one
//! pushes ([`App::push_session_panels`]). The push runs at the end of every
//! `App::update` and at the end of `push_all`, so every way of selecting a
//! session, and every arrival of stats, reaches the panels in the same frame.
//! It remembers what it pushed and pushes only what changed, because
//! `set_attr` asks for a redraw.

use super::{App, Id};
use crate::components::{SESSION_TIME_ATTR, SESSION_TOKENS_ATTR, SessionTimes, payload_value};
use crate::model::{SessionId, SessionStats, TokenUsage};

/// What the panels show: nothing, or the figures of one session.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct PanelValues {
    times: Option<SessionTimes>,
    tokens: Option<TokenUsage>,
}

impl PanelValues {
    /// The values for a session's `stats`; empty while they are not known.
    fn of(stats: Option<SessionStats>) -> Self {
        Self {
            times: stats.map(|stats| SessionTimes {
                run_time: stats.run_time,
                agent_time: stats.agent_time,
            }),
            tokens: stats.map(|stats| stats.tokens),
        }
    }
}

impl App {
    /// The session the panels show: the one the Sessions pane has selected.
    ///
    /// While the filtered selector or search results are listed, the rows
    /// are sessions of any project and the selection is the row's: the one
    /// the App keeps for the pane ([`ListedSessions`]), which records what it
    /// pushed and the moves the list reported. Otherwise the selection is the
    /// `App`'s selected session.
    ///
    /// A search whose first answer is pending lists nothing yet: the pane
    /// still holds the rows it was last given and its cursor over them. An
    /// index snapshot in that window replaces `selector.sessions()` but not
    /// those rows (the new search request is what refreshes them), so the
    /// id is taken from the rows the App last gave the pane, not from its
    /// data.
    ///
    /// [`ListedSessions`]: super::selector::ListedSessions
    fn panels_session(&self) -> Option<SessionId> {
        let listed = &self.selector.listed;
        if self.selector.filtered.is_some() {
            return self.selector.shown.get(listed.selected()?).cloned();
        }
        if self.selector.search_lists_results() {
            return self.selected_result();
        }
        if self.selector.search_shows_results() {
            return listed.selected_id().cloned();
        }
        self.selector.session.clone()
    }

    /// Pushes the stats of the selected session into the panels, or empties
    /// them, when what they show would change.
    ///
    /// Nothing happens while the selector is not mounted: the record of what
    /// was pushed is cleared when it is mounted again (its panels are empty
    /// then), and the next push fills them.
    pub(super) fn push_session_panels(&mut self) {
        if !self.application.mounted(&Id::SessionTime) {
            return;
        }
        let stats = self
            .selector
            .current
            .zip(self.panels_session())
            .and_then(|key| self.selector.stats.get(&key).copied());
        let now = PanelValues::of(stats);
        let before = std::mem::replace(&mut self.selector.panels, now);
        if now.times != before.times {
            let value = payload_value(now.times);
            self.set_attr(Id::SessionTime, SESSION_TIME_ATTR, value);
        }
        if now.tokens != before.tokens {
            let value = payload_value(now.tokens);
            self.set_attr(Id::SessionTokens, SESSION_TOKENS_ATTR, value);
        }
    }

    /// Forgets what was pushed: the panels were just mounted, empty, or
    /// unmounted.
    pub(super) fn forget_pushed_panels(&mut self) {
        self.selector.panels = PanelValues::default();
    }
}
