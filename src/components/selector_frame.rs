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
use crate::app::{BrowserMsg, Msg, UserEvent};
use crate::theme::{Role, Theme};

/// The attribute that says whether the resume confirmation is open
/// (`AttrValue::Flag`): it is modal, so a press outside the selector does
/// nothing then.
pub const RESUME_OPEN_ATTR: Attribute = Attribute::Custom("resume_open");

/// Draws the border, sizing and background of the selector overlay and
/// nothing inside it or outside it. It never takes focus.
///
/// A left press outside it (`UserEvent::PointerOutside`) closes the selector
/// over a session with [`BrowserMsg::CloseSelector`], as `Esc` does. When the
/// selector is the base screen there is nothing to return to, and while the
/// resume confirmation is open ([`RESUME_OPEN_ATTR`]) it is modal: the press
/// does nothing then.
///
/// `view` gets the overlay rect as its area. When the selector is the base
/// screen, the rest of the screen is painted by a
/// [`BackgroundFill`](super::BackgroundFill) of its own, drawn first.
///
/// The border is rounded in `ring` and the interior is filled with `surface`.
/// With a transparent theme nothing is painted with `surface` or `background`
/// and the overlay is cleared instead, so text underneath does not show.
#[derive(Debug, Clone)]
pub struct SelectorFrame {
    theme: Theme,
    /// Whether the selector is the base screen, not over a session.
    base: bool,
    /// Whether the resume confirmation is open.
    resume_open: bool,
}

impl SelectorFrame {
    /// A frame in `theme` for the selector as the base screen.
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            base: true,
            resume_open: false,
        }
    }

    /// The same frame for the selector over a session.
    #[must_use]
    pub fn over_session(self) -> Self {
        Self {
            base: false,
            ..self
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
        frame.render_widget(Clear, area);
        frame.render_widget(self.border(), area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            RESUME_OPEN_ATTR => Some(QueryResult::Owned(AttrValue::Flag(self.resume_open))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match (attr, value) {
            (THEME_ATTR, value) => {
                if let Some(theme) = theme_from_value(&value) {
                    self.theme = theme;
                }
            }
            (RESUME_OPEN_ATTR, AttrValue::Flag(open)) => self.resume_open = open,
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

impl AppComponent<Msg, UserEvent> for SelectorFrame {
    /// Only a press outside the selector (see the type) does something.
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let closes = !self.base && !self.resume_open;
        match ev {
            Event::User(UserEvent::PointerOutside) if closes => {
                Some(Msg::Browser(BrowserMsg::CloseSelector))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::BackgroundFill;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;

    fn default_theme() -> Theme {
        builtin::default_theme()
    }

    /// The frame drawn at `overlay` as the app draws it: on the base screen
    /// (`base`) over the screen's own `BackgroundFill`, drawn first.
    fn draw_buffer(
        theme: Theme,
        base: bool,
        width: u16,
        height: u16,
        overlay: Rect,
    ) -> ratatui::buffer::Buffer {
        let mut fill = BackgroundFill::new(theme.clone());
        let mut frame_component = SelectorFrame::new(theme);
        render(width, height, |frame| {
            if base {
                fill.view(frame, frame.area());
            }
            frame_component.view(frame, overlay);
        })
    }

    fn draw(theme: Theme, base: bool, width: u16, height: u16, overlay: Rect) -> String {
        buffer_to_styled(&draw_buffer(theme, base, width, height, overlay))
    }

    #[test]
    fn frame_over_the_base_screen() {
        insta::assert_snapshot!(draw(default_theme(), true, 30, 8, Rect::new(2, 1, 26, 6)));
    }

    #[test]
    fn frame_over_a_session_leaves_the_outside_alone() {
        let theme = default_theme();
        let buffer = draw_buffer(theme.clone(), false, 30, 8, Rect::new(2, 1, 26, 6));
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
        let buffer = draw_buffer(theme.clone(), true, 20, 6, Rect::new(1, 1, 18, 4));
        assert_eq!(buffer[(1, 1)].symbol(), "╭");
        assert_eq!(buffer[(1, 1)].fg, theme.role(Role::Ring));
        assert_eq!(buffer[(2, 2)].bg, theme.role(Role::Surface));
        assert_eq!(buffer[(0, 0)].bg, theme.role(Role::Background));
    }

    #[test]
    fn the_frame_alone_paints_nothing_outside_its_area() {
        let theme = default_theme();
        let mut component = SelectorFrame::new(theme.clone());
        let overlay = Rect::new(2, 1, 26, 6);
        let buffer = render(30, 8, |frame| component.view(frame, overlay));
        for y in 0..8 {
            for x in 0..30 {
                let inside = overlay.contains((x, y).into());
                let painted = buffer[(x, y)].bg != ratatui::style::Color::Reset;
                assert_eq!(painted, inside, "({x}, {y})");
            }
        }
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

    fn outside(frame: &mut SelectorFrame) -> Option<Msg> {
        frame.on(&Event::User(UserEvent::PointerOutside))
    }

    #[test]
    fn a_press_outside_the_selector_over_a_session_closes_it() {
        let mut frame = SelectorFrame::new(default_theme()).over_session();
        assert_eq!(
            outside(&mut frame),
            Some(Msg::Browser(BrowserMsg::CloseSelector))
        );
    }

    #[test]
    fn a_press_outside_the_base_selector_does_nothing() {
        let mut frame = SelectorFrame::new(default_theme());
        assert_eq!(outside(&mut frame), None);
        frame.attr(RESUME_OPEN_ATTR, AttrValue::Flag(false));
        assert_eq!(outside(&mut frame), None);
    }

    #[test]
    fn a_press_outside_during_the_resume_confirmation_does_nothing() {
        let mut frame = SelectorFrame::new(default_theme()).over_session();
        frame.attr(RESUME_OPEN_ATTR, AttrValue::Flag(true));
        assert_eq!(
            frame.query(RESUME_OPEN_ATTR).unwrap().into_attr(),
            AttrValue::Flag(true)
        );
        assert_eq!(outside(&mut frame), None);
        // Once it is answered, a press outside closes the selector again.
        frame.attr(RESUME_OPEN_ATTR, AttrValue::Flag(false));
        assert_eq!(
            outside(&mut frame),
            Some(Msg::Browser(BrowserMsg::CloseSelector))
        );
    }

    #[test]
    fn a_routed_press_or_wheel_step_inside_does_nothing() {
        let mut frame = SelectorFrame::new(default_theme()).over_session();
        for kind in [
            tuirealm::event::MouseEventKind::Down(tuirealm::event::MouseButton::Left),
            tuirealm::event::MouseEventKind::ScrollDown,
        ] {
            let inside = Event::User(UserEvent::Pointer(crate::app::RoutedMouse {
                kind,
                column: 3,
                row: 3,
                area: ratatui::layout::Size::new(26, 6),
            }));
            assert_eq!(frame.on(&inside), None, "{kind:?}");
        }
    }

    #[test]
    fn it_ignores_events_and_commands() {
        let mut component = SelectorFrame::new(default_theme());
        assert_eq!(component.on(&Event::Tick), None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(component.state(), State::None);
    }
}
