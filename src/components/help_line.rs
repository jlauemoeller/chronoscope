//! The help line: context-sensitive key hints and right-aligned status items
//! (SPEC §4.2).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use unicode_width::UnicodeWidthStr;

use super::{THEME_ATTR, payload_from_value, payload_value, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::theme::{Role, Theme, Token};

/// The attribute that replaces the status items (a `Vec<String>` payload, see
/// [`status_value`]).
pub const STATUS_ATTR: Attribute = Attribute::Custom("status");

/// The attribute that replaces the key hints (a `Vec<Hint>` payload, see
/// [`hints_value`]).
pub const HINTS_ATTR: Attribute = Attribute::Custom("hints");

/// The attribute value that replaces the key hints with `hints`.
pub fn hints_value(hints: Vec<Hint>) -> AttrValue {
    payload_value(hints)
}

/// The attribute value that replaces the status items with `items`.
pub fn status_value(items: Vec<String>) -> AttrValue {
    payload_value(items)
}

/// What separates two hints.
const HINT_SEPARATOR: &str = " | ";
/// What separates two status items.
const STATUS_SEPARATOR: &str = "  ";
/// The least space between the last hint and the status items.
const STATUS_GAP: usize = 2;

/// One key hint: the label of the key and what it does, e.g. `/` and
/// `to search sessions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    /// The key label, drawn in `help_key`.
    pub key: String,
    /// What the key does, drawn in `help_text`.
    pub text: String,
}

impl Hint {
    /// A hint for `key` that reads `<key> <text>`.
    pub fn new(key: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            text: text.into(),
        }
    }

    fn width(&self) -> usize {
        self.key.width() + 1 + self.text.width()
    }
}

/// Draws key hints on the last row of its area, with status items
/// right-aligned. It never takes focus.
///
/// Key labels use `help_key`; hint text, the ` | ` separators and the status
/// items use `help_text`. When everything does not fit, whole hints are
/// dropped from the end; status items are never dropped for hints.
///
/// Before drawing, the whole area is painted with the `fill` role, unless the
/// theme is transparent. With an [`inset`](Self::inset) the text starts that
/// many columns inside the area and ends as many before its right edge, while
/// the fill still covers the whole width.
#[derive(Debug, Clone)]
pub struct HelpLine {
    hints: Vec<Hint>,
    status: Vec<String>,
    theme: Theme,
    fill: Role,
    inset: u16,
}

impl HelpLine {
    /// A help line with `hints` and `status` items, painted over `fill`.
    pub fn new(hints: Vec<Hint>, status: Vec<String>, theme: Theme, fill: Role) -> Self {
        Self {
            hints,
            status,
            theme,
            fill,
            inset: 0,
        }
    }

    /// Keeps `columns` blank columns free on both sides of the text.
    #[must_use]
    pub fn inset(mut self, columns: u16) -> Self {
        self.inset = columns;
        self
    }

    /// The width of the status items with their separators.
    fn status_width(&self) -> usize {
        let items: usize = self.status.iter().map(|s| s.width()).sum();
        items + STATUS_SEPARATOR.len() * self.status.len().saturating_sub(1)
    }

    /// How many hints, counted from the first, fit in `width` columns next to
    /// the status items.
    fn visible_hints(&self, width: usize) -> usize {
        let status = self.status_width();
        let reserved = if status == 0 { 0 } else { status + STATUS_GAP };
        let available = width.saturating_sub(reserved);
        let mut used = 0;
        let mut count = 0;
        for hint in &self.hints {
            let sep = if count == 0 { 0 } else { HINT_SEPARATOR.len() };
            used += sep + hint.width();
            if used > available {
                break;
            }
            count += 1;
        }
        count
    }

    fn hint_line(&self, count: usize) -> Line<'static> {
        let key = self.theme.style(Token::HelpKey);
        let text = self.theme.style(Token::HelpText);
        let mut spans = Vec::new();
        for (i, hint) in self.hints.iter().take(count).enumerate() {
            if i > 0 {
                spans.push(Span::styled(HINT_SEPARATOR, text));
            }
            spans.push(Span::styled(hint.key.clone(), key));
            spans.push(Span::styled(format!(" {}", hint.text), text));
        }
        Line::from(spans)
    }

    fn paint_fill(&self, frame: &mut Frame, area: Rect) {
        if !self.theme.transparent_background() {
            let style = Style::new().bg(self.theme.role(self.fill));
            frame.render_widget(Block::new().style(style), area);
        }
    }

    fn draw_status(&self, frame: &mut Frame, row: Rect) {
        let width = self.status_width().min(usize::from(row.width));
        if width == 0 {
            return;
        }
        // `width` is at most `row.width`, so it fits in a `u16`.
        let width = width as u16;
        let rect = Rect::new(row.right() - width, row.y, width, 1);
        let text = self.status.join(STATUS_SEPARATOR);
        let style = self.theme.style(Token::HelpText);
        frame.render_widget(Paragraph::new(text).style(style), rect);
    }
}

impl Component for HelpLine {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        self.paint_fill(frame, area);
        let inset = self.inset.min(area.width / 2);
        let row = Rect::new(area.x + inset, area.bottom() - 1, area.width - 2 * inset, 1);
        let count = self.visible_hints(usize::from(row.width));
        frame.render_widget(Paragraph::new(self.hint_line(count)), row);
        self.draw_status(frame, row);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            STATUS_ATTR => Some(QueryResult::Owned(status_value(self.status.clone()))),
            HINTS_ATTR => Some(QueryResult::Owned(hints_value(self.hints.clone()))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == THEME_ATTR {
            if let Some(theme) = theme_from_value(&value) {
                self.theme = theme;
            }
        } else if attr == STATUS_ATTR
            && let Some(items) = payload_from_value::<Vec<String>>(&value)
        {
            self.status = items;
        } else if attr == HINTS_ATTR
            && let Some(hints) = payload_from_value::<Vec<Hint>>(&value)
        {
            self.hints = hints;
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for HelpLine {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;

    fn hints() -> Vec<Hint> {
        vec![
            Hint::new("/", "to search sessions"),
            Hint::new("ENTER", "to open"),
            Hint::new("TAB", "to switch between projects and sessions"),
            Hint::new("ESC", "to Cancel"),
        ]
    }

    fn line(status: &[&str]) -> HelpLine {
        HelpLine::new(
            hints(),
            status.iter().map(|s| (*s).to_owned()).collect(),
            builtin::default_theme(),
            Role::Surface,
        )
    }

    fn draw(mut component: HelpLine, width: u16) -> ratatui::buffer::Buffer {
        render(width, 3, |frame| {
            component.view(frame, Rect::new(0, 1, width, 1));
        })
    }

    fn row(buffer: &ratatui::buffer::Buffer, y: u16) -> String {
        let text = buffer_to_text(buffer);
        text.lines()
            .nth(usize::from(y))
            .unwrap_or_default()
            .to_owned()
    }

    const FULL: &str = "/ to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel";

    #[test]
    fn all_hints_are_drawn_when_they_fit() {
        let buffer = draw(line(&[]), 100);
        assert_eq!(row(&buffer, 1).trim_end(), FULL);
    }

    #[test]
    fn full_line_text_and_styles() {
        insta::assert_snapshot!(buffer_to_styled(&draw(line(&[]), 100)));
    }

    #[test]
    fn keys_use_help_key_and_text_uses_help_text() {
        let theme = builtin::default_theme();
        let buffer = draw(line(&[]), 100);
        let key = theme.style(Token::HelpKey).fg.unwrap();
        let text = theme.style(Token::HelpText).fg.unwrap();
        // `/` is column 0, the separator columns 20..23 and `ENTER` 23..28.
        for x in [0, 23, 24, 25, 26, 27] {
            assert_eq!(buffer[(x, 1)].fg, key, "column {x}");
        }
        for x in [2, 19, 20, 21, 22, 28, 29] {
            assert_eq!(buffer[(x, 1)].fg, text, "column {x}");
        }
        assert_eq!(buffer[(0, 1)].bg, theme.role(Role::Surface));
    }

    #[test]
    fn narrow_width_40_with_status_keeps_one_hint() {
        let buffer = draw(line(&["3/17"]), 40);
        insta::assert_snapshot!(buffer_to_text(&buffer));
        assert!(row(&buffer, 1).starts_with("/ to search sessions "));
        assert!(row(&buffer, 1).ends_with("3/17"));
        assert!(!row(&buffer, 1).contains("ENTER"));
    }

    #[test]
    fn width_60_with_status_keeps_two_hints() {
        let buffer = draw(line(&["3/17"]), 60);
        insta::assert_snapshot!(buffer_to_text(&buffer));
        assert!(row(&buffer, 1).contains("ENTER to open"));
        assert!(!row(&buffer, 1).contains("TAB"));
        assert!(row(&buffer, 1).ends_with("3/17"));
    }

    #[test]
    fn hints_are_dropped_whole_never_cut() {
        for width in 0..=100u16 {
            let text = row(&draw(line(&[]), width), 1);
            let drawn = text.trim_end();
            let ok = [
                "",
                "/ to search sessions",
                "/ to search sessions | ENTER to open",
                "/ to search sessions | ENTER to open | TAB to switch between projects and sessions",
                FULL,
            ];
            assert!(ok.contains(&drawn), "width {width}: {drawn:?}");
        }
    }

    #[test]
    fn status_items_are_right_aligned_and_separated_by_two_spaces() {
        let buffer = draw(line(&["3/17", "↓ 2 new"]), 100);
        assert!(row(&buffer, 1).ends_with("3/17  ↓ 2 new"));
        let theme = builtin::default_theme();
        assert_eq!(buffer[(99, 1)].fg, theme.style(Token::HelpText).fg.unwrap());
    }

    #[test]
    fn status_items_are_never_dropped_for_hints() {
        let buffer = draw(line(&["Loading… 34%"]), 20);
        assert!(row(&buffer, 1).ends_with("Loading… 34%"));
        assert!(!row(&buffer, 1).contains('/'));
    }

    #[test]
    fn a_status_wider_than_the_area_is_clipped_without_panic() {
        let buffer = draw(line(&["a very long status item"]), 10);
        assert_eq!(row(&buffer, 1), "a very lon");
    }

    #[test]
    fn text_is_drawn_on_the_last_row_of_the_area() {
        let mut component = line(&[]);
        let buffer = render(100, 5, |frame| {
            component.view(frame, Rect::new(0, 1, 100, 3));
        });
        assert_eq!(row(&buffer, 1).trim(), "");
        assert_eq!(row(&buffer, 2).trim(), "");
        assert_eq!(row(&buffer, 3).trim_end(), FULL);
        let surface = builtin::default_theme().role(Role::Surface);
        assert_eq!(buffer[(50, 1)].bg, surface, "the whole area is filled");
        assert_eq!(buffer[(50, 0)].bg, ratatui::style::Color::Reset);
        assert_eq!(buffer[(50, 4)].bg, ratatui::style::Color::Reset);
    }

    #[test]
    fn the_fill_role_is_painted_behind_the_area() {
        let theme = builtin::default_theme();
        let component = HelpLine::new(hints(), Vec::new(), theme.clone(), Role::Background);
        let buffer = draw(component, 100);
        assert_eq!(buffer[(99, 1)].bg, theme.role(Role::Background));
    }

    #[test]
    fn a_transparent_theme_paints_nothing() {
        let theme = builtin::default_theme().with_transparent_background(true);
        let component = HelpLine::new(hints(), vec!["3/17".into()], theme, Role::Surface);
        let buffer = draw(component, 100);
        for cell in buffer.content() {
            assert_eq!(cell.bg, ratatui::style::Color::Reset);
        }
    }

    #[test]
    fn degenerate_areas_do_not_panic() {
        let mut component = line(&["3/17"]);
        let _ = render(10, 3, |frame| {
            component.view(frame, Rect::new(0, 0, 0, 0));
            component.view(frame, Rect::new(5, 2, 50, 5));
            component.view(frame, Rect::new(0, 0, 10, 0));
        });
    }

    #[test]
    fn the_theme_can_be_replaced_through_an_attribute() {
        let mut component = line(&[]);
        let other = builtin::terminal_theme();
        component.attr(THEME_ATTR, theme_value(&other));
        let queried = component.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&queried), Some(other));
    }

    #[test]
    fn the_status_items_are_replaced_through_an_attribute() {
        let mut component = line(&["3/17"]);
        component.attr(STATUS_ATTR, status_value(vec!["Loading… 34%".to_owned()]));
        let buffer = draw(component.clone(), 100);
        assert!(row(&buffer, 1).ends_with("Loading… 34%"));
        assert!(!row(&buffer, 1).contains("3/17"));
        let queried = component.query(STATUS_ATTR).unwrap().into_attr();
        assert_eq!(
            payload_from_value::<Vec<String>>(&queried),
            Some(vec!["Loading… 34%".to_owned()])
        );
        component.attr(STATUS_ATTR, status_value(Vec::new()));
        assert!(!row(&draw(component, 100), 1).contains("Loading"));
    }

    #[test]
    fn an_inset_keeps_the_text_inside_but_the_fill_covers_the_area() {
        let theme = builtin::default_theme();
        let mut component = line(&["3/17"]).inset(1);
        let buffer = render(100, 2, |frame| component.view(frame, frame.area()));
        let last = row(&buffer, 1);
        assert!(last.starts_with(" / to search sessions"), "{last:?}");
        assert!(last.ends_with("3/17 "), "{last:?}");
        for x in [0, 99] {
            assert_eq!(buffer[(x, 1)].bg, theme.role(Role::Surface), "column {x}");
            assert_eq!(buffer[(x, 0)].bg, theme.role(Role::Surface), "column {x}");
        }
    }

    #[test]
    fn it_ignores_events_and_commands() {
        let mut component = line(&[]);
        assert_eq!(component.on(&Event::Tick), None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(component.state(), State::None);
    }
}
