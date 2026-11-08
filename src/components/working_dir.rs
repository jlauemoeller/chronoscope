//! The Working Directory box of the session selector (product §3.1).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::text_fit::truncate_start;
use super::titled_block::titled_block;
use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::theme::{Role, Theme};

/// The attribute that sets the directory shown (an `AttrValue::String`).
pub const CWD_ATTR: Attribute = Attribute::Custom("cwd");

/// The title of the box.
const TITLE: &str = "Working Directory";
/// The column of the box the directory starts in.
const TEXT_COLUMN: u16 = 3;
/// The rows above the directory inside the box: the border and a blank row.
const ROWS_ABOVE_TEXT: u16 = 2;

/// A rounded box in `border` titled `Working Directory` that shows the
/// working directory of the selected project in `foreground`. A directory
/// too long for the box is cut from the left. It never takes focus.
#[derive(Debug, Clone)]
pub struct WorkingDir {
    theme: Theme,
    cwd: String,
}

impl WorkingDir {
    /// A box showing `cwd`.
    pub fn new(cwd: impl Into<String>, theme: Theme) -> Self {
        Self {
            theme,
            cwd: cwd.into(),
        }
    }

    /// The columns the directory can use: from column 3 to three cells before
    /// the right border.
    fn text_width(area: Rect) -> usize {
        usize::from(area.width).saturating_sub(2 * usize::from(TEXT_COLUMN))
    }
}

impl Component for WorkingDir {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        frame.render_widget(titled_block(&self.theme, TITLE, Role::Border), area);
        if area.height <= ROWS_ABOVE_TEXT + 1 {
            return;
        }
        let width = Self::text_width(area);
        let text = truncate_start(&self.cwd, width);
        let style = Style::new().fg(self.theme.role(Role::Foreground));
        let rect = Rect::new(
            area.x + TEXT_COLUMN,
            area.y + ROWS_ABOVE_TEXT,
            u16::try_from(width).unwrap_or(0),
            1,
        );
        frame.render_widget(Paragraph::new(text).style(style), rect);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            CWD_ATTR => Some(QueryResult::Owned(AttrValue::String(self.cwd.clone()))),
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
            (CWD_ATTR, AttrValue::String(cwd)) => self.cwd = cwd,
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

impl AppComponent<Msg, UserEvent> for WorkingDir {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;

    fn draw(cwd: &str, width: u16) -> ratatui::buffer::Buffer {
        let mut component = WorkingDir::new(cwd, builtin::default_theme());
        render(width, 5, |frame| component.view(frame, frame.area()))
    }

    #[test]
    fn box_with_a_directory_snapshot() {
        let buffer = draw("/home/jacob/Development/projects/Calira", 60);
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn the_directory_starts_in_column_3_below_a_blank_row() {
        let text = buffer_to_text(&draw("/home/jacob", 40));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[0],
            format!("╭─ Working Directory {}╮", "─".repeat(18))
        );
        assert_eq!(lines[1], format!("│{}│", " ".repeat(38)));
        assert_eq!(lines[2], format!("│  /home/jacob{}│", " ".repeat(25)));
        assert_eq!(lines[3], format!("│{}│", " ".repeat(38)));
    }

    #[test]
    fn a_long_directory_is_cut_from_the_left_with_an_ellipsis() {
        let text = buffer_to_text(&draw("/home/jacob/Development/projects/Calira", 30));
        let line = text.lines().nth(2).unwrap();
        // Text width is 30 - 6 = 24 columns.
        assert_eq!(line, "│  …lopment/projects/Calira  │");
        assert_eq!(line.chars().count(), 30);
    }

    #[test]
    fn the_directory_is_foreground_and_the_border_is_border() {
        let theme = builtin::default_theme();
        let buffer = draw("/x", 30);
        assert_eq!(buffer[(3, 2)].fg, theme.role(Role::Foreground));
        assert_eq!(buffer[(0, 0)].fg, theme.role(Role::Border));
        assert_eq!(buffer[(0, 2)].fg, theme.role(Role::Border));
    }

    #[test]
    fn the_directory_is_set_through_an_attribute() {
        let mut component = WorkingDir::new("", builtin::default_theme());
        component.attr(CWD_ATTR, AttrValue::String("/p".to_owned()));
        assert_eq!(
            component.query(CWD_ATTR).unwrap().into_attr(),
            AttrValue::String("/p".to_owned())
        );
        assert!(
            buffer_to_text(&{
                let mut c = component.clone();
                render(30, 5, |frame| c.view(frame, frame.area()))
            })
            .contains("│  /p")
        );
    }

    #[test]
    fn it_ignores_events_and_a_tiny_area_does_not_panic() {
        let mut component = WorkingDir::new("/x", builtin::default_theme());
        assert_eq!(component.on(&Event::Tick), None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(component.state(), State::None);
        for (w, h) in [(0, 0), (1, 1), (5, 2), (7, 4), (30, 3)] {
            let _ = render(30, 6, |frame| component.view(frame, Rect::new(0, 0, w, h)));
        }
    }
}
