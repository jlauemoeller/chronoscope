//! The Session Time panel of the session selector (product §3.2, §3.3): the
//! selected session's run time and agent time, or an empty block.

use std::time::Duration;

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::panel_row::PanelRows;
use super::titled_block::titled_block;
use super::{THEME_ATTR, payload_from_value, payload_value, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::duration::format_duration;
use crate::theme::{Role, Theme};

/// The attribute that sets the durations shown: a payload of
/// `Option<SessionTimes>`, `None` for the empty block.
pub const SESSION_TIME_ATTR: Attribute = Attribute::Custom("session_time");

/// The title of the panel.
const TITLE: &str = "Session Time";

/// The durations of a session the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTimes {
    /// The session's run time (`Total`).
    pub run_time: Duration,
    /// The sum of its Assistant turns' durations (`Agent`), as on its row.
    pub agent_time: Duration,
}

/// A rounded box in `border` titled `Session Time`, below the Sessions pane,
/// with a `Total` and an `Agent` row, or no rows while it has no durations.
/// It never takes focus and ignores every event.
#[derive(Debug, Clone)]
pub struct SessionTime {
    theme: Theme,
    times: Option<SessionTimes>,
}

impl SessionTime {
    /// A panel showing `times`, or an empty one for `None`.
    pub fn new(times: Option<SessionTimes>, theme: Theme) -> Self {
        Self { theme, times }
    }

    /// The `(label, value)` rows, in order.
    fn rows(times: SessionTimes) -> Vec<(&'static str, String)> {
        vec![
            ("Total", format_duration(times.run_time)),
            ("Agent", format_duration(times.agent_time)),
        ]
    }
}

impl Component for SessionTime {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        frame.render_widget(titled_block(&self.theme, TITLE, Role::Border), area);
        if let Some(times) = self.times {
            frame.render_widget(PanelRows::new(Self::rows(times), &self.theme), area);
        }
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            SESSION_TIME_ATTR => Some(QueryResult::Owned(payload_value(self.times))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match attr {
            THEME_ATTR => {
                if let Some(theme) = theme_from_value(&value) {
                    self.theme = theme;
                }
            }
            SESSION_TIME_ATTR => {
                if let Some(times) = payload_from_value::<Option<SessionTimes>>(&value) {
                    self.times = times;
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
        draw_times(None, width, theme)
    }

    fn draw_times(
        times: Option<SessionTimes>,
        width: u16,
        theme: Theme,
    ) -> ratatui::buffer::Buffer {
        let mut component = SessionTime::new(times, theme);
        render(width, HEIGHT, |frame| component.view(frame, frame.area()))
    }

    /// The worked example: 81h 33m in all, 17h 56m of agent time.
    fn example() -> Option<SessionTimes> {
        Some(SessionTimes {
            run_time: Duration::from_secs(293_580),
            agent_time: Duration::from_secs(64_560),
        })
    }

    #[test]
    fn with_times_default_theme_at_width_31_snapshot() {
        let buffer = draw_times(example(), 31, builtin::default_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn with_times_default_theme_at_width_75_snapshot() {
        let buffer = draw_times(example(), 75, builtin::default_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn with_times_terminal_theme_at_width_31_snapshot() {
        let buffer = draw_times(example(), 31, builtin::terminal_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn with_times_terminal_theme_at_width_75_snapshot() {
        let buffer = draw_times(example(), 75, builtin::terminal_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn a_narrow_panel_cuts_the_label_and_keeps_the_value() {
        // 11 columns: a row has 7 cells, the value takes 7 and no label fits.
        let text = buffer_to_text(&draw_times(example(), 11, builtin::default_theme()));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "│ 81h 33m │");
        // 14 columns: 10 cells, 7 for the value, a blank and 2 for the label.
        let text = buffer_to_text(&draw_times(example(), 14, builtin::default_theme()));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "│ T… 81h 33m │");
        assert_eq!(lines[2], "│ A… 17h 56m │");
        insta::assert_snapshot!(text);
    }

    #[test]
    fn the_worked_example_reads_as_in_the_wireframe() {
        let text = buffer_to_text(&draw_times(example(), 56, builtin::default_theme()));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], format!("│ Total{}81h 33m │", " ".repeat(40)));
        assert_eq!(lines[2], format!("│ Agent{}17h 56m │", " ".repeat(40)));
    }

    #[test]
    fn labels_are_muted_and_values_foreground() {
        let theme = builtin::default_theme();
        assert_ne!(
            theme.role(Role::MutedForeground),
            theme.role(Role::Foreground)
        );
        let buffer = draw_times(example(), 31, theme.clone());
        for y in [1, 2] {
            for x in 2..7 {
                assert_eq!(
                    buffer[(x, y)].fg,
                    theme.role(Role::MutedForeground),
                    "{x},{y}"
                );
            }
            for x in 22..29 {
                assert_eq!(buffer[(x, y)].fg, theme.role(Role::Foreground), "{x},{y}");
            }
            assert_eq!(buffer[(0, y)].fg, theme.role(Role::Border));
            assert_eq!(buffer[(30, y)].fg, theme.role(Role::Border));
        }
    }

    #[test]
    fn the_agent_value_is_the_duration_of_the_row() {
        let agent = Duration::from_millis(3_900_999);
        let times = Some(SessionTimes {
            run_time: Duration::from_secs(9_999),
            agent_time: agent,
        });
        let text = buffer_to_text(&draw_times(times, 31, builtin::default_theme()));
        let line = text.lines().nth(2).unwrap();
        assert!(
            line.ends_with(&format!("{} │", format_duration(agent))),
            "{line}"
        );
    }

    #[test]
    fn the_times_are_set_through_an_attribute_and_none_clears_them() {
        let mut component = SessionTime::new(None, builtin::default_theme());
        let query = |c: &SessionTime| {
            payload_from_value::<Option<SessionTimes>>(
                &c.query(SESSION_TIME_ATTR).unwrap().into_attr(),
            )
            .unwrap()
        };
        assert_eq!(query(&component), None);
        component.attr(SESSION_TIME_ATTR, payload_value(example()));
        assert_eq!(query(&component), example());
        let buffer = render(31, HEIGHT, |frame| component.view(frame, frame.area()));
        assert!(buffer_to_text(&buffer).contains("81h 33m"));
        component.attr(SESSION_TIME_ATTR, payload_value(None::<SessionTimes>));
        assert_eq!(query(&component), None);
        let buffer = render(31, HEIGHT, |frame| component.view(frame, frame.area()));
        assert!(!buffer_to_text(&buffer).contains("Total"));
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
        let mut component = SessionTime::new(example(), builtin::default_theme());
        let nord = builtin::get("nord").expect("nord is built in");
        component.attr(THEME_ATTR, theme_value(&nord));
        let queried = component.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&queried), Some(nord.clone()));
        let buffer = render(31, HEIGHT, |frame| component.view(frame, frame.area()));
        assert_eq!(buffer[(0, 0)].fg, nord.role(Role::Border));
    }

    #[test]
    fn it_ignores_events_and_a_tiny_area_does_not_panic() {
        let mut component = SessionTime::new(example(), builtin::default_theme());
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
