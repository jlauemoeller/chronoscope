//! A centred message that fills a whole area (SPEC §4.2).

use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Paragraph};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::{THEME_ATTR, theme_from_value, theme_value, truncate_with_ellipsis};
use crate::app::{Msg, UserEvent};
use crate::theme::{Role, Theme};

/// The message shown when the terminal is too small.
pub const TOO_SMALL: &str = "Terminal too small";
/// The message shown in the Sessions pane when the harness has nothing to
/// list.
pub const NO_SESSIONS: &str = "No sessions for this harness";

/// Shows a message centred horizontally and vertically in its area, in
/// `muted_foreground` on `background` (the background is not painted when the
/// theme is transparent). It never takes focus.
///
/// Inside another component (the Sessions pane) it is built
/// [`without_background`](Self::without_background), so it leaves what is
/// behind it as it is.
#[derive(Debug, Clone)]
pub struct EmptyState {
    message: String,
    theme: Theme,
    paint_background: bool,
}

impl EmptyState {
    /// An empty state showing `message`.
    pub fn new(message: impl Into<String>, theme: Theme) -> Self {
        Self {
            message: message.into(),
            theme,
            paint_background: true,
        }
    }

    /// Leaves the cells around the message as they are instead of painting
    /// `background` over the area.
    #[must_use]
    pub fn without_background(mut self) -> Self {
        self.paint_background = false;
        self
    }

    fn text_style(&self) -> Style {
        Style::new().fg(self.theme.role(Role::MutedForeground))
    }

    fn background_style(&self) -> Style {
        if self.theme.transparent_background() || !self.paint_background {
            Style::new()
        } else {
            Style::new().bg(self.theme.role(Role::Background))
        }
    }
}

impl Component for EmptyState {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        frame.render_widget(Block::new().style(self.background_style()), area);
        let middle = Rect {
            y: area.y + area.height / 2,
            height: 1,
            ..area
        };
        let message = truncate_with_ellipsis(&self.message, usize::from(area.width));
        let text = Paragraph::new(message)
            .alignment(Alignment::Center)
            .style(self.text_style());
        frame.render_widget(text, middle);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Text => Some(QueryResult::Owned(AttrValue::String(self.message.clone()))),
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (Attribute::Text, AttrValue::String(message)) => self.message = message,
            (THEME_ATTR, value) => {
                if let Some(theme) = theme_from_value(&value) {
                    self.theme = theme;
                }
            }
            _ => {}
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for EmptyState {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;

    fn draw(theme: Theme, width: u16, height: u16) -> ratatui::buffer::Buffer {
        let mut component = EmptyState::new(TOO_SMALL, theme);
        render(width, height, |frame| {
            let area = frame.area();
            component.view(frame, area);
        })
    }

    #[test]
    fn too_small_message_text() {
        insta::assert_snapshot!(buffer_to_text(&draw(builtin::default_theme(), 40, 9)));
    }

    #[test]
    fn too_small_message_styled() {
        insta::assert_snapshot!(buffer_to_styled(&draw(builtin::default_theme(), 40, 9)));
    }

    #[test]
    fn transparent_theme_paints_no_background() {
        let theme = builtin::default_theme().with_transparent_background(true);
        insta::assert_snapshot!(buffer_to_styled(&draw(theme, 40, 5)));
    }

    #[test]
    fn message_is_centred_on_the_middle_row() {
        for (w, h, row) in [(59u16, 24u16, 12usize), (80, 19, 9), (40, 1, 0)] {
            let buffer = draw(builtin::default_theme(), w, h);
            let text = buffer_to_text(&buffer);
            let found: Vec<usize> = text
                .lines()
                .enumerate()
                .filter(|(_, l)| l.contains(TOO_SMALL))
                .map(|(i, _)| i)
                .collect();
            assert_eq!(found, vec![row], "{w}x{h}");
            let line = text.lines().nth(row).unwrap();
            let left = line.find(TOO_SMALL).unwrap();
            let right = usize::from(w) - left - TOO_SMALL.len();
            assert!(left.abs_diff(right) <= 1, "{w}x{h}: {left} vs {right}");
        }
    }

    #[test]
    fn text_is_muted_foreground_on_background() {
        let theme = builtin::default_theme();
        let buffer = draw(theme.clone(), 40, 9);
        let cell = &buffer[(11, 4)];
        assert_eq!(cell.symbol(), "T");
        assert_eq!(cell.fg, theme.role(Role::MutedForeground));
        assert_eq!(cell.bg, theme.role(Role::Background));
        assert_eq!(buffer[(0, 0)].bg, theme.role(Role::Background));
    }

    #[test]
    fn a_narrow_area_truncates_instead_of_panicking() {
        let buffer = draw(builtin::default_theme(), 5, 3);
        assert_eq!(buffer.area, Rect::new(0, 0, 5, 3));
        let _ = draw(builtin::default_theme(), 0, 0);
    }

    #[test]
    fn message_and_theme_can_be_replaced_through_attributes() {
        let mut component = EmptyState::new("a", builtin::default_theme());
        component.attr(Attribute::Text, AttrValue::String("b".to_owned()));
        assert_eq!(
            component.query(Attribute::Text).unwrap().into_attr(),
            AttrValue::String("b".to_owned())
        );
        let other = builtin::terminal_theme();
        component.attr(THEME_ATTR, theme_value(&other));
        let queried = component.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&queried), Some(other));
    }

    #[test]
    fn it_ignores_events_and_commands() {
        let mut component = EmptyState::new(TOO_SMALL, builtin::default_theme());
        assert_eq!(component.on(&Event::Tick), None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(component.state(), State::None);
    }
}
