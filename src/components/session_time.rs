//! The Session Time panel of the session selector (SPEC §4.1, §4.2): a titled empty block. The App will push the selected session's time totals into it.

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::list_pane::titled_block;
use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::theme::{Role, Theme};

/// The title of the panel.
const TITLE: &str = "Session Time";

/// A rounded box in `border` titled `Session Time`, below the Sessions pane. It
/// never takes focus and ignores every event.
#[derive(Debug, Clone)]
pub struct SessionTime {
    theme: Theme,
}

impl SessionTime {
    /// An empty panel.
    pub fn new(theme: Theme) -> Self {
        Self { theme }
    }
}

impl Component for SessionTime {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        frame.render_widget(titled_block(&self.theme, TITLE, Role::Border), area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            _ => None,
        }
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

impl AppComponent<Msg, UserEvent> for SessionTime {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};

    const HEIGHT: u16 = 4;

    fn draw(width: u16, theme: Theme) -> ratatui::buffer::Buffer {
        let mut component = SessionTime::new(theme);
        render(width, HEIGHT, |frame| component.view(frame, frame.area()))
    }

    #[test]
    fn default_theme_at_width_31_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(31, builtin::default_theme())));
    }

    #[test]
    fn default_theme_at_width_75_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(75, builtin::default_theme())));
    }

    #[test]
    fn terminal_theme_at_width_31_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(31, builtin::terminal_theme())));
    }

    #[test]
    fn terminal_theme_at_width_75_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(75, builtin::terminal_theme())));
    }

    #[test]
    fn it_is_an_empty_rounded_box_with_its_title() {
        let text = buffer_to_text(&draw(31, builtin::default_theme()));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), usize::from(HEIGHT));
        assert_eq!(
            lines[0],
            format!("╭─ {TITLE} {}╮", "─".repeat(31 - 5 - TITLE.len()))
        );
        for line in &lines[1..lines.len() - 1] {
            assert_eq!(*line, format!("│{}│", " ".repeat(29)));
        }
        assert_eq!(lines[lines.len() - 1], format!("╰{}╯", "─".repeat(29)));
    }

    #[test]
    fn the_border_is_border_and_the_title_is_foreground() {
        let theme = builtin::default_theme();
        let buffer = draw(31, theme.clone());
        assert_eq!(buffer[(0, 0)].fg, theme.role(Role::Border));
        assert_eq!(buffer[(0, 1)].fg, theme.role(Role::Border));
        assert_eq!(buffer[(30, HEIGHT - 1)].fg, theme.role(Role::Border));
        assert_eq!(buffer[(3, 0)].fg, theme.role(Role::Foreground));
        assert_ne!(theme.role(Role::Border), theme.role(Role::Ring));
    }

    #[test]
    fn the_theme_is_set_through_an_attribute() {
        let mut component = SessionTime::new(builtin::default_theme());
        let nord = builtin::get("nord").expect("nord is built in");
        component.attr(THEME_ATTR, theme_value(&nord));
        let queried = component.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&queried), Some(nord.clone()));
        let buffer = render(31, HEIGHT, |frame| component.view(frame, frame.area()));
        assert_eq!(buffer[(0, 0)].fg, nord.role(Role::Border));
    }

    #[test]
    fn it_ignores_events_and_a_tiny_area_does_not_panic() {
        let mut component = SessionTime::new(builtin::default_theme());
        assert_eq!(component.on(&Event::Tick), None);
        let key = Event::Keyboard(KeyEvent::new(Key::Enter, KeyModifiers::NONE));
        assert_eq!(component.on(&key), None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(component.state(), State::None);
        for (w, h) in [
            (0, 0),
            (1, 1),
            (2, 2),
            (5, 1),
            (31, 1),
            (3, HEIGHT),
            (31, 40),
        ] {
            let _ = render(31, 8, |frame| component.view(frame, Rect::new(0, 0, w, h)));
        }
    }
}
