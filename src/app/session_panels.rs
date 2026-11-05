//! The Session Time and Session Tokens panels' values (SPEC §4.3): which
//! session they show, and pushing its stats into them.
//!
//! One function works out the session ([`App::panels_session`]) and one
//! pushes ([`App::push_session_panels`]). The push runs at the end of every
//! `App::update` and at the end of `push_all`, so every way of selecting a
//! session, and every arrival of stats, reaches the panels in the same frame.
//! It remembers what it pushed and pushes only what changed, because
//! `set_attr` asks for a redraw.

use super::{App, Id};
use crate::components::{
    SESSION_ROWS_ATTR, SESSION_TIME_ATTR, SESSION_TOKENS_ATTR, SessionRows, SessionTimes,
    payload_from_value, payload_value,
};
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
    /// While the filtered selector or search results are listed, the list
    /// moves its own cursor over sessions of any project and reports no
    /// message, so it is asked; otherwise the selection is the `App`'s.
    ///
    /// A search whose first answer is pending lists nothing yet: the pane
    /// still holds the rows it was last given and its cursor over them. An
    /// index snapshot in that window replaces `selector.sessions()` but not
    /// those rows (the new search request is what refreshes them), so the
    /// id is taken from the rows the list holds, not from the `App`'s data.
    fn panels_session(&self) -> Option<SessionId> {
        if self.selector.filtered.is_some() {
            let row = self.selected_row()?;
            return self.selector.shown.get(row).cloned();
        }
        if self.selector.search_lists_results() {
            return self.selected_result();
        }
        if self.selector.search_shows_results() {
            let rows = self.session_list_rows()?;
            return rows.rows.get(rows.selected?).map(|row| row.id.clone());
        }
        self.selector.session.clone()
    }

    /// The rows the Sessions pane holds, with its cursor.
    fn session_list_rows(&self) -> Option<SessionRows> {
        let value = self
            .application
            .query(&Id::SessionList, SESSION_ROWS_ATTR)
            .ok()??
            .into_attr();
        payload_from_value(&value)
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
