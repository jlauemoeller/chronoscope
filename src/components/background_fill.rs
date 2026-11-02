//! A strip of the screen that belongs to no pane but must not be left
//! unpainted, such as the column between the turns sidebar and the
//! transcript (SPEC §6.1).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::theme::{Role, Theme};

/// Paints its area with `background`, or nothing when the theme is
/// transparent (SPEC §8.1). It draws no text, takes no focus and no events.
#[derive(Debug, Clone)]
pub struct BackgroundFill {
    theme: Theme,
}

impl BackgroundFill {
    /// A fill in `theme`.
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }
}

impl Component for BackgroundFill {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() || self.theme.transparent_background() {
            return;
        }
        let style = Style::new().bg(self.theme.role(Role::Background));
        frame.render_widget(Block::new().style(style), area);
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

    fn perform(&mut self, cmd: Cmd) -> CmdResult {
        CmdResult::Invalid(cmd)
    }
}

impl AppComponent<Msg, UserEvent> for BackgroundFill {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use tuirealm::event::{Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

    use super::*;
    use crate::test_support::{buffer_to_styled, render};
    use crate::theme::builtin;

    fn draw(theme: Theme) -> ratatui::buffer::Buffer {
        let mut fill = BackgroundFill::new(theme);
        render(5, 4, |frame| fill.view(frame, Rect::new(1, 1, 1, 2)))
    }

    #[test]
    fn it_paints_only_its_area_with_the_background() {
        let theme = builtin::default_theme();
        let buffer = draw(theme.clone());
        let background = theme.role(Role::Background);
        assert_eq!(buffer[(1, 1)].bg, background);
        assert_eq!(buffer[(1, 2)].bg, background);
        for (x, y) in [(0, 1), (2, 1), (1, 0), (1, 3), (4, 3)] {
            assert_eq!(buffer[(x, y)].bg, Color::Reset, "({x}, {y})");
        }
    }

    #[test]
    fn a_transparent_theme_paints_nothing() {
        let buffer = draw(builtin::default_theme().with_transparent_background(true));
        for y in 0..4 {
            for x in 0..5 {
                assert_eq!(buffer[(x, y)].bg, Color::Reset, "({x}, {y})");
            }
        }
    }

    #[test]
    fn the_theme_is_replaced_through_an_attribute() {
        let mut fill = BackgroundFill::new(builtin::default_theme());
        let other = builtin::terminal_theme();
        fill.attr(THEME_ATTR, theme_value(&other));
        let value = fill.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&value), Some(other));
    }

    #[test]
    fn the_fill_in_the_default_theme() {
        insta::assert_snapshot!(buffer_to_styled(&draw(builtin::default_theme())));
    }

    #[test]
    fn the_fill_in_a_transparent_theme() {
        insta::assert_snapshot!(buffer_to_styled(&draw(
            builtin::default_theme().with_transparent_background(true)
        )));
    }

    #[test]
    fn it_ignores_events_and_commands() {
        let mut fill = BackgroundFill::new(builtin::default_theme());
        let key = Event::Keyboard(KeyEvent::new(Key::Char('j'), KeyModifiers::NONE));
        let mouse = Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: KeyModifiers::NONE,
            column: 1,
            row: 1,
        });
        for event in [Event::Tick, key, mouse] {
            assert_eq!(fill.on(&event), None);
        }
        assert_eq!(fill.perform(Cmd::None), CmdResult::Invalid(Cmd::None));
        assert_eq!(fill.state(), State::None);
    }
}
