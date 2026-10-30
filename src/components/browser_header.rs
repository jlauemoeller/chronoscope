//! The header of the session browser (SPEC §4.2, §9.5): the project and the
//! session id in a rounded block over the working directory.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use unicode_width::UnicodeWidthStr;

use super::working_dir::truncate_start;
use super::{THEME_ATTR, theme_from_value, theme_value, truncate_with_ellipsis};
use crate::app::{Msg, UserEvent};
use crate::theme::{Role, Theme, Token};

/// The attribute that sets how many entries of the session could not be
/// parsed (an `AttrValue::Length`).
pub const PARSE_WARNINGS_ATTR: Attribute = Attribute::Custom("parse_warnings");

/// The attribute that sets whether the `● live` marker is shown (an
/// `AttrValue::Flag`).
pub const LIVE_ATTR: Attribute = Attribute::Custom("live");

/// The header is this many rows high: the border, a blank row, the working
/// directory, a blank row and the border.
pub const HEADER_HEIGHT: u16 = 5;
/// The columns between the cwd (or the note) and the border: a blank column.
const PADDING: usize = 1;
/// The columns the working directory starts after the left border.
const CWD_COLUMN: usize = 2;
/// The project name keeps at least this many columns before the session id
/// is dropped to make room for it.
const MIN_NAME_WIDTH: usize = 6;
/// The least number of cells between the working directory and what is
/// right-aligned, and between the note and the live marker.
const NOTE_GAP: usize = 2;
/// The dot of the live marker, drawn in `primary`.
const LIVE_DOT: &str = "●";
/// The word after the dot, drawn in `muted_foreground`.
const LIVE_WORD: &str = " live";

/// The note for `count` entries that could not be parsed, or `None` for 0.
pub fn parse_warning_note(count: usize) -> Option<String> {
    match count {
        0 => None,
        1 => Some("1 entry could not be parsed".to_owned()),
        n => Some(format!("{n} entries could not be parsed")),
    }
}

/// A rounded block in `border`: `╭─ Project <name> ─…─ <session id> ─╮`, a
/// blank row, the working directory (cut from the left when it is too long),
/// a blank row and the bottom border. When entries of the session could not
/// be parsed, a note is right-aligned on the working directory's row. While
/// the session is live, `● live` (`●` in `primary`, `live` in
/// `muted_foreground`) is right-aligned there too, with the note two cells to
/// its left.
///
/// `Project` is `muted_foreground`, the project name bold `foreground`, the
/// session id `timestamp`, the working directory `foreground` and the note
/// `muted_foreground`. The area is first painted with `background`, unless
/// the theme is transparent. It never takes focus.
#[derive(Debug, Clone)]
pub struct BrowserHeader {
    project: String,
    session_id: String,
    cwd: String,
    parse_warnings: usize,
    live: bool,
    theme: Theme,
}

impl BrowserHeader {
    /// A header for the session `session_id` of the project `project`, whose
    /// working directory is `cwd`.
    pub fn new(
        project: impl Into<String>,
        session_id: impl Into<String>,
        cwd: impl Into<String>,
        theme: Theme,
    ) -> Self {
        Self {
            project: project.into(),
            session_id: session_id.into(),
            cwd: cwd.into(),
            parse_warnings: 0,
            live: false,
            theme,
        }
    }

    /// Sets whether the `● live` marker is shown.
    #[must_use]
    pub fn live(mut self, live: bool) -> Self {
        self.live = live;
        self
    }

    /// Sets how many entries could not be parsed.
    #[must_use]
    pub fn parse_warnings(mut self, count: usize) -> Self {
        self.parse_warnings = count;
        self
    }

    fn border(&self) -> Style {
        Style::new().fg(self.theme.role(Role::Border))
    }

    fn muted(&self) -> Style {
        Style::new().fg(self.theme.role(Role::MutedForeground))
    }

    fn foreground(&self) -> Style {
        Style::new().fg(self.theme.role(Role::Foreground))
    }

    /// The top border for `width` columns.
    ///
    /// The project name is cut with `…` to leave room for the session id; on
    /// a width where even a short name does not fit next to it, the id goes.
    fn top(&self, width: usize) -> Line<'static> {
        let border = self.border();
        // `╭─`, the titles, at least one `─`, `─╮`.
        let room = width.saturating_sub(5);
        let mut id = format!(" {} ", self.session_id);
        // " Project " before the name and a blank after it.
        let chrome = "Project".width() + 3;
        let mut name_room = room.saturating_sub(chrome + id.width());
        if name_room < MIN_NAME_WIDTH.min(self.project.width()) {
            id.clear();
            name_room = room.saturating_sub(chrome);
        }
        let name = if name_room == 0 {
            String::new()
        } else {
            truncate_with_ellipsis(&self.project, name_room)
        };
        let used = chrome + name.width() + id.width();
        let fill = width.saturating_sub(4 + used);
        let mut spans = vec![
            Span::styled("╭─", border),
            Span::styled(" Project ", self.muted()),
            Span::styled(name, self.foreground().add_modifier(Modifier::BOLD)),
            Span::styled(" ", self.muted()),
            Span::styled("─".repeat(fill), border),
        ];
        if !id.is_empty() {
            spans.push(Span::styled(id, self.theme.style(Token::Timestamp)));
        }
        spans.push(Span::styled("─╮", border));
        Line::from(spans)
    }

    /// A body row of `width` columns holding only the borders.
    fn blank(&self, width: usize) -> Line<'static> {
        let border = self.border();
        Line::from(vec![
            Span::styled("│", border),
            Span::raw(" ".repeat(width.saturating_sub(2))),
            Span::styled("│", border),
        ])
    }

    /// The row with the working directory, the note and the live marker.
    fn cwd_row(&self, width: usize) -> Line<'static> {
        let border = self.border();
        let inner = width.saturating_sub(2);
        let note = parse_warning_note(self.parse_warnings);
        let note_width = note.as_deref().map_or(0, UnicodeWidthStr::width);
        let live_width = if self.live {
            LIVE_DOT.width() + LIVE_WORD.width()
        } else {
            0
        };
        let gap = if note.is_some() && self.live {
            NOTE_GAP
        } else {
            0
        };
        // What is right-aligned, and the blank column before the border.
        let right = note_width + gap + live_width;
        let trailing = if right > 0 { right + PADDING } else { 0 };
        // One blank column before the border, and the columns before the
        // directory.
        let reserved = if right > 0 { right + NOTE_GAP } else { 0 };
        let available = inner.saturating_sub(CWD_COLUMN - 1 + PADDING + reserved);
        let cwd = truncate_start(&self.cwd, available);
        let cwd_width = cwd.width();
        let before = inner.saturating_sub(CWD_COLUMN - 1 + cwd_width + trailing);
        let mut spans = vec![
            Span::styled("│", border),
            Span::raw(" ".repeat((CWD_COLUMN - 1).min(inner))),
            Span::styled(cwd, self.foreground()),
            Span::raw(" ".repeat(before)),
        ];
        if let Some(note) = note {
            spans.push(Span::styled(note, self.muted()));
        }
        if gap > 0 {
            spans.push(Span::raw(" ".repeat(gap)));
        }
        if self.live {
            let dot = Style::new().fg(self.theme.role(Role::Primary));
            spans.push(Span::styled(LIVE_DOT, dot));
            spans.push(Span::styled(LIVE_WORD, self.muted()));
        }
        if right > 0 {
            spans.push(Span::raw(" ".repeat(PADDING.min(inner))));
        }
        spans.push(Span::styled("│", border));
        Line::from(spans)
    }

    fn bottom(&self, width: usize) -> Line<'static> {
        let fill = "─".repeat(width.saturating_sub(2));
        Line::from(Span::styled(format!("╰{fill}╯"), self.border()))
    }
}

impl Component for BrowserHeader {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        if !self.theme.transparent_background() {
            let style = Style::new().bg(self.theme.role(Role::Background));
            frame.render_widget(Block::new().style(style), area);
        }
        if area.width < 2 {
            return;
        }
        let width = usize::from(area.width);
        let lines = vec![
            self.top(width),
            self.blank(width),
            self.cwd_row(width),
            self.blank(width),
            self.bottom(width),
        ];
        frame.render_widget(Paragraph::new(lines), area);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            PARSE_WARNINGS_ATTR => Some(QueryResult::Owned(AttrValue::Length(self.parse_warnings))),
            LIVE_ATTR => Some(QueryResult::Owned(AttrValue::Flag(self.live))),
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
            (PARSE_WARNINGS_ATTR, AttrValue::Length(count)) => self.parse_warnings = count,
            (LIVE_ATTR, AttrValue::Flag(live)) => self.live = live,
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

impl AppComponent<Msg, UserEvent> for BrowserHeader {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;
    use ratatui::style::Color;

    const SESSION: &str = "85a86657-11a7-4318-a21a-0f4476babd5c";
    const CWD: &str = "/home/user/projects/brio";

    fn header(warnings: usize) -> BrowserHeader {
        BrowserHeader::new("BRIO", SESSION, CWD, builtin::default_theme()).parse_warnings(warnings)
    }

    fn rows(header: &mut BrowserHeader, width: u16) -> Vec<String> {
        let buffer = render(width, HEADER_HEIGHT, |frame| {
            header.view(frame, frame.area());
        });
        buffer_to_text(&buffer).lines().map(str::to_owned).collect()
    }

    fn distinct_theme() -> Theme {
        builtin::default_theme()
            .with_role(Role::Foreground, Color::Rgb(250, 1, 1))
            .with_role(Role::MutedForeground, Color::Rgb(2, 250, 2))
            .with_role(Role::Border, Color::Rgb(3, 3, 250))
            .with_role(Role::Background, Color::Rgb(9, 9, 9))
    }

    #[test]
    fn the_header_is_a_five_row_block_with_both_titles() {
        let rows = rows(&mut header(0), 120);
        assert_eq!(rows.len(), 5);
        let top = format!(
            "╭─ Project BRIO {} {SESSION} ─╮",
            "─".repeat(120 - 4 - 14 - 38),
        );
        assert_eq!(rows[0], top);
        assert_eq!(rows[0].chars().count(), 120);
        assert_eq!(rows[1], format!("│{}│", " ".repeat(118)));
        assert_eq!(rows[2], format!("│ {CWD:<117}│"));
        assert_eq!(rows[3], rows[1]);
        assert_eq!(rows[4], format!("╰{}╯", "─".repeat(118)));
    }

    #[test]
    fn the_working_directory_starts_in_column_2() {
        let rows = rows(&mut header(0), 80);
        assert_eq!(rows[2].chars().nth(2), Some('/'));
        assert_eq!(rows[2].chars().nth(1), Some(' '));
    }

    #[test]
    fn three_unparsed_entries_are_noted_on_the_cwd_row() {
        let rows = rows(&mut header(3), 120);
        let note = "3 entries could not be parsed";
        assert!(rows[2].ends_with(&format!("{note} │")), "{}", rows[2]);
        assert_eq!(rows[2].chars().count(), 120);
        // The note ends in column w - 3, then a blank column and the border.
        let chars: Vec<char> = rows[2].chars().collect();
        assert_eq!(chars[117], 'd');
        assert_eq!(chars[118], ' ');
        assert_eq!(chars[119], '│');
    }

    #[test]
    fn one_unparsed_entry_is_singular() {
        let rows = rows(&mut header(1), 120);
        assert!(
            rows[2].contains("1 entry could not be parsed │"),
            "{}",
            rows[2]
        );
        assert!(!rows[2].contains("entries"));
    }

    #[test]
    fn no_warnings_means_no_note() {
        let rows = rows(&mut header(0), 120);
        assert!(!rows[2].contains("parsed"));
        assert_eq!(parse_warning_note(0), None);
    }

    #[test]
    fn a_long_cwd_is_cut_from_the_left_and_leaves_two_cells_before_the_note() {
        let cwd = "/home/jacob/Development/projects/clients/very-long-customer-name/monorepo";
        let mut h =
            BrowserHeader::new("BRIO", SESSION, cwd, builtin::default_theme()).parse_warnings(3);
        let rows = rows(&mut h, 80);
        let row = &rows[2];
        assert_eq!(row.chars().count(), 80);
        assert!(row.starts_with("│ …"), "{row}");
        let note = "3 entries could not be parsed";
        let note_start = row.find(note).unwrap();
        let before: Vec<char> = row[..note_start].chars().collect();
        let blanks = before.iter().rev().take_while(|c| **c == ' ').count();
        assert_eq!(blanks, 2, "{row}");
        assert!(row.contains("monorepo"), "the end of the path stays: {row}");
    }

    #[test]
    fn a_long_cwd_without_a_note_is_cut_to_the_box() {
        let cwd = "/".to_owned() + &"a".repeat(200);
        let mut h = BrowserHeader::new("BRIO", SESSION, cwd, builtin::default_theme());
        let rows = rows(&mut h, 60);
        assert_eq!(rows[2].chars().count(), 60);
        assert!(rows[2].starts_with("│ …"));
        assert!(rows[2].ends_with("a │"), "{}", rows[2]);
    }

    #[test]
    fn a_long_project_name_is_cut_with_an_ellipsis_and_keeps_the_session_id() {
        let name = "A".repeat(100);
        let mut h = BrowserHeader::new(name, SESSION, CWD, builtin::default_theme());
        let rows = rows(&mut h, 80);
        assert_eq!(rows[0].chars().count(), 80);
        assert!(rows[0].contains('…'), "{}", rows[0]);
        assert!(rows[0].contains(SESSION), "{}", rows[0]);
        assert!(rows[0].ends_with("─╮"));
    }

    #[test]
    fn the_parse_warnings_are_set_through_an_attribute() {
        let mut h = header(0);
        h.attr(PARSE_WARNINGS_ATTR, AttrValue::Length(3));
        assert_eq!(
            h.query(PARSE_WARNINGS_ATTR).unwrap().into_attr(),
            AttrValue::Length(3)
        );
        assert!(rows(&mut h, 120)[2].contains("3 entries could not be parsed"));
        h.attr(PARSE_WARNINGS_ATTR, AttrValue::Length(1));
        assert!(rows(&mut h, 120)[2].contains("1 entry could not be parsed"));
        h.attr(PARSE_WARNINGS_ATTR, AttrValue::Length(0));
        assert!(!rows(&mut h, 120)[2].contains("parsed"));
    }

    #[test]
    fn the_live_marker_ends_in_column_w_minus_3_on_the_cwd_row() {
        for width in [60u16, 80, 120] {
            let mut h = header(0).live(true);
            let rows = rows(&mut h, width);
            let chars: Vec<char> = rows[2].chars().collect();
            let w = usize::from(width);
            assert_eq!(chars.len(), w);
            assert_eq!(chars[w - 3], 'e', "{}", rows[2]);
            assert_eq!(chars[w - 2], ' ');
            assert_eq!(chars[w - 1], '│');
            assert_eq!(chars[w - 8..w - 2].iter().collect::<String>(), "● live");
        }
    }

    #[test]
    fn without_the_flag_there_is_no_live_marker() {
        let rows = rows(&mut header(0), 120);
        assert!(!rows[2].contains("live"));
    }

    #[test]
    fn the_note_sits_two_cells_left_of_the_live_marker() {
        let mut h = header(2).live(true);
        let rows = rows(&mut h, 120);
        let tail = "2 entries could not be parsed  ● live │";
        assert!(rows[2].ends_with(tail), "{}", rows[2]);
        assert_eq!(rows[2].chars().count(), 120);
        assert!(rows[2].starts_with("│ /home/user/projects/brio"));
    }

    #[test]
    fn a_long_cwd_is_cut_to_leave_room_for_the_note_and_the_marker() {
        let cwd = "/home/jacob/Development/projects/clients/very-long-customer-name/monorepo";
        let mut h = BrowserHeader::new("BRIO", SESSION, cwd, builtin::default_theme())
            .parse_warnings(3)
            .live(true);
        let rows = rows(&mut h, 80);
        let row = &rows[2];
        assert_eq!(row.chars().count(), 80);
        assert!(row.starts_with("│ …"), "{row}");
        assert!(
            row.ends_with("3 entries could not be parsed  ● live │"),
            "{row}"
        );
        let before = &row[..row.find("3 entries").unwrap()];
        assert_eq!(before.chars().rev().take_while(|c| *c == ' ').count(), 2);
    }

    #[test]
    fn the_live_flag_is_set_through_an_attribute() {
        let mut h = header(0);
        assert_eq!(
            h.query(LIVE_ATTR).unwrap().into_attr(),
            AttrValue::Flag(false)
        );
        h.attr(LIVE_ATTR, AttrValue::Flag(true));
        assert_eq!(
            h.query(LIVE_ATTR).unwrap().into_attr(),
            AttrValue::Flag(true)
        );
        assert!(rows(&mut h, 120)[2].contains("● live"));
        h.attr(LIVE_ATTR, AttrValue::Flag(false));
        assert!(!rows(&mut h, 120)[2].contains("live"));
    }

    #[test]
    fn the_dot_is_primary_and_the_word_is_muted_foreground() {
        let theme = distinct_theme().with_role(Role::Primary, Color::Rgb(7, 200, 7));
        let mut h = BrowserHeader::new("BRIO", SESSION, CWD, theme.clone())
            .parse_warnings(3)
            .live(true);
        let buffer = render(120, 5, |frame| h.view(frame, frame.area()));
        let at = |x: u16| &buffer[(x, 2)];
        assert_eq!(at(112).symbol(), "●");
        assert_eq!(at(112).fg, theme.role(Role::Primary));
        for x in 114..=117 {
            assert_eq!(at(x).fg, theme.role(Role::MutedForeground), "col {x}");
        }
        assert_eq!(at(117).symbol(), "e");
        assert_eq!(
            at(120 - 4 - 12).fg,
            theme.role(Role::MutedForeground),
            "note"
        );
    }

    #[test]
    fn titles_cwd_and_note_have_their_own_colours() {
        let theme = distinct_theme();
        let timestamp = theme.style(Token::Timestamp).fg.unwrap();
        let mut h = BrowserHeader::new("BRIO", SESSION, CWD, theme.clone()).parse_warnings(3);
        let buffer = render(120, 5, |frame| h.view(frame, frame.area()));
        let fg = |x: u16, y: u16| buffer[(x, y)].fg;
        assert_eq!(fg(0, 0), theme.role(Role::Border), "corner");
        assert_eq!(fg(3, 0), theme.role(Role::MutedForeground), "Project");
        assert_eq!(fg(11, 0), theme.role(Role::Foreground), "name");
        assert!(buffer[(11, 0)].modifier.contains(Modifier::BOLD));
        assert!(!buffer[(3, 0)].modifier.contains(Modifier::BOLD));
        assert_eq!(fg(81, 0), timestamp, "session id");
        assert_eq!(fg(2, 2), theme.role(Role::Foreground), "cwd");
        assert_eq!(fg(120 - 4, 2), theme.role(Role::MutedForeground), "note");
        assert_eq!(fg(0, 2), theme.role(Role::Border), "side border");
        for y in 0..5 {
            for x in 0..120 {
                assert_eq!(buffer[(x, y)].bg, theme.role(Role::Background), "{x},{y}");
            }
        }
    }

    #[test]
    fn a_transparent_theme_paints_no_background() {
        let theme = builtin::default_theme().with_transparent_background(true);
        let mut h = BrowserHeader::new("BRIO", SESSION, CWD, theme);
        let buffer = render(80, 5, |frame| h.view(frame, frame.area()));
        assert_eq!(buffer[(40, 1)].bg, Color::Reset);
    }

    #[test]
    fn header_with_a_note_styled_snapshot() {
        let mut h = header(3);
        let buffer = render(120, 5, |frame| h.view(frame, frame.area()));
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn it_ignores_events_and_tiny_areas_do_not_panic() {
        let mut h = header(3);
        assert_eq!(h.on(&Event::Tick), None);
        assert_eq!(h.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(h.state(), State::None);
        for (w, ht) in [(0, 0), (1, 1), (2, 2), (5, 3), (10, 5), (30, 5)] {
            let _ = render(30, 6, |frame| h.view(frame, Rect::new(0, 0, w, ht)));
        }
    }
}
