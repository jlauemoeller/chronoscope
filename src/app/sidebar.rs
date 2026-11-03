//! The turns sidebar's focus and keys in the app (SPEC §6.1, §6.3, §6.4):
//! which keys hand the focus to it and back, what selecting an entry does to
//! the transcript, what happens to the focus when the sidebar is hidden, and
//! the hints the help line shows while it has the focus.
//!
//! The sidebar's selection follows the transcript's while the transcript
//! has the focus ([`App::sync_sidebar_selection`](super::App)); while the
//! sidebar has it, the sidebar moves the transcript instead.

use ratatui::layout::Rect;

use super::hints::{browser_hints_at, turns_hints};
use super::layout::{browser_areas, too_small};
use super::redraw::RedrawRequest;
use super::{App, Id};
use crate::components::{GO_TO_TURN_ATTR, HINTS_ATTR, Hint, hints_value, payload_value};
use crate::model::TurnId;

impl App {
    /// Whether the turns sidebar is on screen: the browser is shown, at least
    /// 90 columns wide, and the session is not empty (SPEC §6.1).
    pub(super) fn sidebar_shown(&self) -> bool {
        if !self.screen.has_browser()
            || self.browser.session_empty()
            || !self.application.mounted(&Id::TurnSidebar)
        {
            return false;
        }
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        !too_small(area) && browser_areas(area).sidebar.is_some()
    }

    /// Whether the sidebar has the focus, or an overlay or the selector that
    /// it opened does (who had the focus before the first of them is
    /// `focus_stack`'s first entry). That is the focus the sidebar's
    /// selection and the help line's hints go by.
    pub(super) fn sidebar_has_focus(&self) -> bool {
        self.focus_stack
            .first()
            .or_else(|| self.application.focus())
            == Some(&Id::TurnSidebar)
    }

    /// `1`, `Tab` or `Shift-Tab` in the transcript: gives the sidebar the
    /// focus. The transcript does not move. Nothing happens while the sidebar
    /// is hidden.
    pub(super) fn focus_turns(&mut self) {
        if !self.sidebar_shown() || self.application.focus() == Some(&Id::TurnSidebar) {
            return;
        }
        if let Err(err) = self.application.active(&Id::TurnSidebar) {
            log::warn!("could not focus the turns sidebar: {err}");
            return;
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// `Tab`, `Shift-Tab`, `→` or `l` in the sidebar: returns the focus to
    /// the transcript on top, leaving it as it is.
    pub(super) fn focus_transcript_from_sidebar(&mut self) {
        if self.application.focus() != Some(&Id::TurnSidebar) {
            return;
        }
        self.focus_top_transcript();
    }

    fn focus_top_transcript(&mut self) {
        let top = self.top_transcript();
        if let Err(err) = self.application.active(&top) {
            log::warn!("could not return the focus to {top:?}: {err}");
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// The sidebar selected the entry of turn `id` (`open`: and `Enter` was
    /// pressed): scrolls the transcript on top to its block with the result
    /// `]` / `[` give, and, on `Enter`, gives it the focus. Whether the view
    /// still follows the end, and `↓ N new`, are the transcript's to decide
    /// and are read back.
    pub(super) fn go_to_turn(&mut self, id: &TurnId, open: bool) {
        if !self.sidebar_shown() {
            return;
        }
        let top = self.top_transcript();
        self.set_attr(top, GO_TO_TURN_ATTR, payload_value(id.clone()));
        self.sync_new_sections();
        if open {
            self.focus_top_transcript();
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Moves the focus off a sidebar that is hidden (the screen got narrower
    /// than 90 columns or too small, or the session turned out empty): to the
    /// transcript on top. The focus that is *remembered* for an overlay or the
    /// selector that was opened from the sidebar moves too, or closing them
    /// would hand the focus to a pane that is not there.
    pub(super) fn release_hidden_sidebar(&mut self) {
        if self.sidebar_shown() || !self.application.mounted(&Id::TurnSidebar) {
            return;
        }
        let top = self.top_transcript();
        for remembered in &mut self.focus_stack {
            if *remembered == Id::TurnSidebar {
                *remembered = top;
            }
        }
        if self.application.focus() == Some(&Id::TurnSidebar) {
            self.focus_top_transcript();
        }
    }

    /// The hints the help line shows now: those of the sidebar while it has
    /// the focus, else the browser's at the depth of the transcript on top.
    /// Remembers which, for [`refresh_hints`](Self::refresh_hints).
    pub(super) fn push_hints(&mut self) {
        let sidebar = self.sidebar_has_focus();
        self.browser.sidebar_hints = sidebar;
        let hints: Vec<Hint> = if sidebar {
            turns_hints(&self.keymap)
        } else {
            browser_hints_at(&self.keymap, self.browser.depth())
        };
        self.set_attr(Id::BrowserHelpLine, HINTS_ATTR, hints_value(hints));
    }

    /// Changes the help line's hints when the sidebar gained or lost the
    /// focus since they were pushed. Runs after every message, so every path
    /// that moves the focus is covered once: keys, a click on the
    /// transcript, the sidebar being hidden, an overlay closing.
    pub(super) fn refresh_hints(&mut self) {
        if !self.application.mounted(&Id::BrowserHelpLine) {
            return;
        }
        if self.sidebar_has_focus() != self.browser.sidebar_hints {
            self.push_hints();
        }
    }
}
