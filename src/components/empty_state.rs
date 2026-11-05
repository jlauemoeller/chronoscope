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
/// The message shown in the browser when the session has no turns.
pub const EMPTY_SESSION: &str = "Session file is empty";
/// The first line of the browser's empty state when the session holds only
/// local commands; the commands follow, one per line.
pub const LOCAL_COMMANDS_ONLY: &str = "No turns. Local commands only:";
/// The message shown in the Sessions pane when the harness has nothing to
/// list.
pub const NO_SESSIONS: &str = "No sessions for this harness";
/// The message shown in the Sessions pane when a search matches nothing.
pub const NO_MATCHING_SESSIONS: &str = "No matching sessions";

/// The message of the browser's empty state: [`LOCAL_COMMANDS_ONLY`] and the
/// `commands`, one per line, when there are any, else [`EMPTY_SESSION`].
pub fn empty_session_message(commands: &[String]) -> String {
    if commands.is_empty() {
        return EMPTY_SESSION.to_owned();
    }
    let mut message = LOCAL_COMMANDS_ONLY.to_owned();
    for command in commands {
        message.push('\n');
        message.push_str(command);
    }
    message
}

/// Shows a message centred horizontally and vertically in its area, in
/// `muted_foreground` on `background` (the background is not painted when the
/// theme is transparent). It never takes focus.
///
/// A message of several lines (split on `\n`) is centred as a block: with `n`
/// lines the first is on the middle row less `(n - 1) / 2`, but not above the
/// area, and each line is centred on its own. A line too wide for the area
/// ends in `…`, and lines that do not fit below the area are not drawn.
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

    /// Draws `line` centred on row `row` of `area`.
    fn draw_line(&self, frame: &mut Frame, area: Rect, row: u16, line: &str) {
        let at = Rect {
            y: area.y + row,
            height: 1,
            ..area
        };
        let line = truncate_with_ellipsis(line, usize::from(area.width));
        let text = Paragraph::new(line)
            .alignment(Alignment::Center)
            .style(self.text_style());
        frame.render_widget(text, at);
    }
}

impl Component for EmptyState {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        frame.render_widget(Block::new().style(self.background_style()), area);
        let lines: Vec<&str> = self.message.split('\n').collect();
        let middle = area.height / 2;
        let first = middle.saturating_sub(((lines.len() - 1) / 2).try_into().unwrap_or(u16::MAX));
        for (line, row) in lines.into_iter().zip(first..area.height) {
            self.draw_line(frame, area, row, line);
        }
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
    fn a_three_line_message_text() {
        let mut component = EmptyState::new(
            "No turns. Local commands only:\n/effort\n/clear",
            builtin::default_theme(),
        );
        let buffer = render(44, 9, |frame| {
            let area = frame.area();
            component.view(frame, area);
        });
        insta::assert_snapshot!(buffer_to_text(&buffer));
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

    fn draw_message(message: &str, area: Rect, width: u16, height: u16) -> Vec<String> {
        let mut component = EmptyState::new(message, builtin::default_theme());
        let buffer = render(width, height, |frame| component.view(frame, area));
        buffer_to_text(&buffer)
            .lines()
            .map(|l| l.trim_end().to_owned())
            .collect()
    }

    #[test]
    fn a_one_line_message_is_on_the_middle_row_of_an_area_below_the_top() {
        let rows = draw_message("Hello", Rect::new(0, 3, 20, 9), 20, 14);
        let at: Vec<usize> = (0..rows.len())
            .filter(|&i| rows[i].contains("Hello"))
            .collect();
        assert_eq!(at, [3 + 9 / 2]);
    }

    #[test]
    fn several_lines_are_centred_as_a_block_each_on_its_own() {
        let area = Rect::new(0, 0, 21, 9);
        let rows = draw_message("Heading here\nab\nabcdefghij", area, 21, 9);
        // Three lines: the first is one row above the middle row (4).
        assert_eq!(rows[2], "");
        assert_eq!(rows[3].trim(), "Heading here");
        assert_eq!(rows[4].trim(), "ab");
        assert_eq!(rows[5].trim(), "abcdefghij");
        assert_eq!(rows[6], "");
        // Each is centred on its own: 21 - 12 = 9 cells around the first.
        assert_eq!(rows[3].find("Heading"), Some(4));
        assert_eq!(rows[4].find("ab"), Some(9));
        assert_eq!(rows[5].find("abc"), Some(5));
    }

    #[test]
    fn an_even_number_of_lines_puts_the_extra_line_below_the_middle() {
        let rows = draw_message("a\nb\nc\nd", Rect::new(0, 0, 5, 10), 5, 10);
        // Four lines: the first is (4 - 1) / 2 = 1 row above the middle (5).
        let at: Vec<usize> = (0..rows.len()).filter(|&i| !rows[i].is_empty()).collect();
        assert_eq!(at, [4, 5, 6, 7]);
    }

    #[test]
    fn lines_that_do_not_fit_below_the_area_are_not_drawn() {
        let message = (1..=10).map(|n| format!("line {n}")).collect::<Vec<_>>();
        let rows = draw_message(&message.join("\n"), Rect::new(0, 2, 12, 5), 12, 10);
        // The block starts at the top of the area (row 2), not above it.
        let drawn: Vec<&str> = rows.iter().map(|r| r.trim()).collect();
        assert_eq!(
            drawn,
            [
                "", "", "line 1", "line 2", "line 3", "line 4", "line 5", "", "", ""
            ]
        );
    }

    #[test]
    fn a_line_wider_than_the_area_ends_in_an_ellipsis() {
        let rows = draw_message(
            "short\nthis line is far too wide",
            Rect::new(0, 0, 10, 3),
            10,
            3,
        );
        assert_eq!(rows[1].trim(), "short");
        assert_eq!(rows[2], "this line…");
    }

    #[test]
    fn tiny_areas_do_not_panic() {
        let many = "a\nb\nc\nd\ne\nf";
        for (w, h) in [(0u16, 0u16), (1, 1), (1, 6), (6, 1), (2, 2), (3, 1)] {
            let mut component = EmptyState::new(many, builtin::default_theme());
            let buffer = render(w, h, |frame| {
                let area = frame.area();
                component.view(frame, area);
            });
            assert_eq!(buffer.area, Rect::new(0, 0, w, h));
        }
        // An area partly outside the frame.
        let mut component = EmptyState::new(many, builtin::default_theme());
        render(4, 3, |frame| component.view(frame, Rect::new(2, 1, 40, 40)));
        let mut component = EmptyState::new("", builtin::default_theme());
        render(4, 3, |frame| {
            let area = frame.area();
            component.view(frame, area);
        });
    }

    #[test]
    fn a_multi_line_message_has_the_same_style_on_every_line() {
        let theme = builtin::default_theme();
        let mut component = EmptyState::new("ab\ncd", theme.clone());
        let buffer = render(9, 5, |frame| {
            let area = frame.area();
            component.view(frame, area);
        });
        // Two lines: the first on row 2 - 0 = 2, the second on row 3.
        for (row, symbol) in [(2u16, "a"), (3, "c")] {
            let cell = &buffer[(3, row)];
            assert_eq!(cell.symbol(), symbol);
            assert_eq!(cell.fg, theme.role(Role::MutedForeground));
            assert_eq!(cell.bg, theme.role(Role::Background));
        }
    }

    #[test]
    fn the_local_commands_message_lists_one_command_per_line() {
        assert_eq!(empty_session_message(&[]), EMPTY_SESSION);
        let commands = ["/effort".to_owned(), "/model opus".to_owned()];
        assert_eq!(
            empty_session_message(&commands),
            "No turns. Local commands only:\n/effort\n/model opus"
        );
        assert!(empty_session_message(&commands).starts_with(LOCAL_COMMANDS_ONLY));
    }
}
