//! The details overlay (SPEC §5): the metadata of one section, in groups of
//! `label  value` rows, in a centred rounded overlay that scrolls when it is
//! taller than the screen.
//!
//! It shares its frame, scrolling and keys with the help overlay (see
//! [`OverlayFrame`]). The content is a snapshot, taken when the overlay
//! opens: nothing updates it while it is open.

mod content;
mod rows;

#[cfg(test)]
mod snapshots;

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};

pub use content::{DetailsContent, DetailsGroup, DetailsRow};
pub use rows::section_details;

use super::overlay_frame::{FrameKey, INTERIOR_ATTR, OverlayFrame, interior_value};
use super::{THEME_ATTR, payload_from_value, theme_from_value, theme_value};
use crate::app::{Msg, RoutedMouse, UserEvent};
use crate::keymap::{Action, Context, Keymap};
use crate::theme::Theme;

/// The details overlay. It takes the focus; `Esc` or the `details` key
/// closes it, the up and down keys scroll a row and the page keys a page.
///
/// `view` gets the overlay's rect (see `App`'s layout) as its area. Values
/// wider than the interior are wrapped at the width the overlay is drawn
/// with; the rows that scrolling counts are those of the announced interior
/// (see [`INTERIOR_ATTR`]).
#[derive(Debug, Clone)]
pub struct DetailsOverlay {
    content: DetailsContent,
    frame: OverlayFrame,
}

impl DetailsOverlay {
    /// An overlay that shows `content`, resolving keys with `keymap`.
    pub fn new(content: DetailsContent, theme: Theme, keymap: Keymap) -> Self {
        Self {
            content,
            frame: OverlayFrame::new("Details", theme, keymap),
        }
    }

    /// The size the overlay wants, borders included, when it may be at most
    /// `max_width` columns wide (see [`DetailsContent::desired_size`]).
    pub fn desired_size(&self, max_width: u16) -> Size {
        self.content.desired_size(max_width)
    }

    /// The rows of the content at the announced interior width; nothing
    /// scrolls until it is announced.
    fn total(&self) -> usize {
        self.frame
            .interior()
            .map_or(0, |size| self.content.row_count(size.width))
    }
}

impl Component for DetailsOverlay {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.frame.theme().clone();
        self.frame
            .draw(frame, area, |width| self.content.lines(&theme, width));
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        if attr == THEME_ATTR {
            Some(QueryResult::Owned(theme_value(self.frame.theme())))
        } else if attr == INTERIOR_ATTR {
            self.frame
                .interior()
                .map(|size| QueryResult::Owned(interior_value(size)))
        } else {
            None
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == THEME_ATTR {
            if let Some(theme) = theme_from_value(&value) {
                self.frame.set_theme(theme);
            }
        } else if attr == INTERIOR_ATTR
            && let Some(size) = payload_from_value::<Size>(&value)
        {
            self.frame
                .set_interior(size, self.content.row_count(size.width));
        }
    }

    /// The first row shown.
    fn state(&self) -> State {
        State::Single(StateValue::Usize(self.frame.offset()))
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl DetailsOverlay {
    /// The wheel scrolls [`WHEEL_ROWS`](super::overlay_frame::WHEEL_ROWS)
    /// rows; clicks do nothing (the `App` closes the overlay for one outside
    /// it).
    fn on_mouse(&mut self, mouse: &RoutedMouse) -> Option<Msg> {
        let total = self.total();
        self.frame
            .on_mouse(mouse, total)
            .then_some(Msg::DetailsScrolled)
    }
}

impl AppComponent<Msg, UserEvent> for DetailsOverlay {
    /// The overlay keys come first (`Esc` closes, the up, down and page keys
    /// scroll); then the `details` key of the browser closes it too. Every
    /// other key does nothing: the keys of the transcript and the global keys
    /// are not for the overlay.
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let key = match ev {
            Event::Keyboard(key) => key,
            // Only what the `App` routed here, in the overlay's coordinates.
            Event::User(UserEvent::Mouse(mouse)) => return self.on_mouse(mouse),
            _ => return None,
        };
        let total = self.total();
        match self.frame.on_key(key, total) {
            FrameKey::Close => Some(Msg::CloseDetails),
            FrameKey::Scrolled => Some(Msg::DetailsScrolled),
            FrameKey::Unmoved => None,
            FrameKey::Unbound => (self.frame.keymap().resolve(Context::Browser, key)
                == Some(Action::Details))
            .then_some(Msg::CloseDetails),
        }
    }
}
