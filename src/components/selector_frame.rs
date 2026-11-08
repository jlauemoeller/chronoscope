//! The session selector overlay's frame (product §3.1, §3.3).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Clear};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::theme::{Role, Theme};

/// Draws the border, sizing and background of the selector overlay and
/// nothing inside it. It never takes focus.
///
/// `view` gets the overlay rect as its area; `frame.area()` is the whole
/// screen. When the selector is the base screen (no session open behind it)
/// the whole screen is first painted with `background`.
///
/// The border is rounded in `ring` and the interior is filled with `surface`.
/// With a transparent theme nothing is painted with `surface` or `background`
/// and the overlay is cleared instead, so text underneath does not show.
#[derive(Debug, Clone)]
pub struct SelectorFrame {
    theme: Theme,
    base: bool,
}

impl SelectorFrame {
    /// A frame over the base screen.
    pub fn new(theme: Theme) -> Self {
        Self { theme, base: true }
    }

    /// Whether the selector is the base screen. When it is not, it is drawn
    /// over a session and the screen is left as it is outside the overlay.
    #[must_use]
    pub fn base(mut self, base: bool) -> Self {
        self.base = base;
        self
    }

    fn paint_background(&self, frame: &mut Frame) {
        if self.base && !self.theme.transparent_background() {
            let style = Style::new().bg(self.theme.role(Role::Background));
            frame.render_widget(Block::new().style(style), frame.area());
        }
    }

    fn border(&self) -> Block<'static> {
        let ring = Style::new().fg(self.theme.role(Role::Ring));
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(ring);
        if self.theme.transparent_background() {
            block
        } else {
            block.style(Style::new().bg(self.theme.role(Role::Surface)))
        }
    }
}

impl Component for SelectorFrame {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        self.paint_background(frame);
        frame.render_widget(Clear, area);
        frame.render_widget(self.border(), area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        (attr == THEME_ATTR).then(|| QueryResult::Owned(theme_value(&self.theme)))
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == THEME_ATTR
            && let Some(theme) = theme_from_value(&value)
        {
            self.theme = theme;
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for SelectorFrame {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;

    fn default_theme() -> Theme {
        builtin::default_theme()
    }

    fn draw(theme: Theme, base: bool, width: u16, height: u16, overlay: Rect) -> String {
        let mut frame_component = SelectorFrame::new(theme).base(base);
        let buffer = render(width, height, |frame| frame_component.view(frame, overlay));
        buffer_to_styled(&buffer)
    }

    #[test]
    fn frame_over_the_base_screen() {
        insta::assert_snapshot!(draw(default_theme(), true, 30, 8, Rect::new(2, 1, 26, 6)));
    }

    #[test]
    fn frame_over_a_session_leaves_the_outside_alone() {
        let theme = default_theme();
        let mut component = SelectorFrame::new(theme.clone()).base(false);
        let buffer = render(30, 8, |frame| component.view(frame, Rect::new(2, 1, 26, 6)));
        assert_eq!(buffer[(0, 0)].bg, ratatui::style::Color::Reset);
        assert_eq!(buffer[(2, 1)].bg, theme.role(Role::Surface));
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn transparent_frame_paints_no_background_or_surface() {
        let theme = default_theme().with_transparent_background(true);
        insta::assert_snapshot!(draw(theme, true, 30, 8, Rect::new(2, 1, 26, 6)));
    }

    #[test]
    fn border_is_ring_and_interior_is_surface() {
        let theme = default_theme();
        let mut component = SelectorFrame::new(theme.clone());
        let buffer = render(20, 6, |frame| component.view(frame, Rect::new(1, 1, 18, 4)));
        assert_eq!(buffer[(1, 1)].symbol(), "╭");
        assert_eq!(buffer[(1, 1)].fg, theme.role(Role::Ring));
        assert_eq!(buffer[(2, 2)].bg, theme.role(Role::Surface));
        assert_eq!(buffer[(0, 0)].bg, theme.role(Role::Background));
    }

    #[test]
    fn text_below_a_transparent_overlay_does_not_bleed_through() {
        let theme = default_theme().with_transparent_background(true);
        let mut component = SelectorFrame::new(theme);
        let buffer = render(20, 6, |frame| {
            let rows = vec!["x".repeat(20); 6].join("\n");
            frame.render_widget(ratatui::widgets::Paragraph::new(rows), frame.area());
            component.view(frame, Rect::new(1, 1, 18, 4));
        });
        let text = buffer_to_text(&buffer);
        assert_eq!(
            text.lines().nth(2).unwrap(),
            format!("x│{}│x", " ".repeat(16))
        );
    }

    #[test]
    fn an_overlay_larger_than_the_screen_is_clipped() {
        let mut component = SelectorFrame::new(default_theme());
        let buffer = render(10, 5, |frame| {
            component.view(frame, Rect::new(0, 0, 50, 50))
        });
        assert_eq!(buffer.area, Rect::new(0, 0, 10, 5));
    }

    #[test]
    fn the_theme_can_be_replaced_through_an_attribute() {
        let mut component = SelectorFrame::new(default_theme());
        let other = builtin::terminal_theme();
        component.attr(THEME_ATTR, theme_value(&other));
        let queried = component.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&queried), Some(other));
    }

    #[test]
    fn it_ignores_events_and_commands() {
        let mut component = SelectorFrame::new(default_theme());
        assert_eq!(component.on(&Event::Tick), None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(component.state(), State::None);
    }
}
