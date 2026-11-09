//! The help-line prompt: search, file paths and `y/n` questions (architecture.md#components,
//! §4.2, §4.3, §6).
//!
//! While a prompt is active it has the focus (input mode): every printable
//! key goes into the text. Only `Enter`, `Esc`, `Backspace`, the arrow keys
//! and `Ctrl-c` keep a meaning, so none of the browser's or selector's keys
//! conflict with typing.

use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Widget};
use tui_input::{Input, InputRequest};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};
use unicode_width::UnicodeWidthChar;

use super::text_fit::truncate_start;
use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::keymap::KeySpec;
use crate::theme::{Role, Theme, Token};

/// What a prompt asks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptMode {
    /// A line of text (a search, a file path).
    Text,
    /// A `y/n` question: `y` or `Y` answers yes, every other key no.
    YesNo,
}

/// A one-row prompt drawn in place of the help line.
///
/// In [`PromptMode::Text`] printable characters are inserted at the cursor
/// (`tui-input` does the editing). Every change of the text is reported as
/// [`Msg::PromptChanged`], a move of the cursor alone as [`Msg::PromptMoved`]
/// (the screen needs a redraw), `Enter` as [`Msg::PromptSubmit`], `Esc` as
/// [`Msg::PromptCancel`], and `↑` / `↓` as [`Msg::PromptNav`] for the owner
/// of the prompt to use. `Ctrl-c` always quits.
///
/// In [`PromptMode::YesNo`] `y` / `Y` is [`Msg::PromptSubmit`] with `"y"` and
/// every other key but `Ctrl-c` is [`Msg::PromptCancel`].
///
/// A `y/n` prompt made with [`Prompt::confirm`] draws its question in
/// `help_text` with the subject (a path) in `foreground`.
///
/// The label is drawn in `help_key`, the text in `foreground`, and the cell
/// under the cursor with the `cursor` colour as its background. Text longer
/// than the row scrolls horizontally to keep the cursor visible. The row is
/// painted with the `fill` role first (the help line's), unless the theme is
/// transparent.
#[derive(Debug, Clone)]
pub struct Prompt {
    label: String,
    input: Input,
    mode: PromptMode,
    theme: Theme,
    fill: Role,
    inset: u16,
    focused: bool,
    question: Option<Question>,
}

/// The parts of a question: `Overwrite `, `./notes.txt`, `? (y/n)`; or
/// `Resume `, `85a86657`, ` in `, `Claude`, `? (y/n)`. The question is drawn
/// in `help_text`, the subjects in `foreground`.
#[derive(Debug, Clone)]
struct Question {
    before: String,
    subject: String,
    /// Between the subjects; empty with one subject.
    between: String,
    /// The second subject; empty with one subject.
    second: String,
    after: String,
}

impl Prompt {
    /// A prompt with `label`, starting with `text` (the cursor after it).
    pub fn new(
        label: impl Into<String>,
        text: impl Into<String>,
        mode: PromptMode,
        theme: Theme,
    ) -> Self {
        Self {
            label: label.into(),
            input: Input::new(text.into()),
            mode,
            theme,
            fill: Role::Surface,
            inset: 0,
            focused: false,
            question: None,
        }
    }

    /// A `y/n` prompt that asks `before` + `subject` + `after` (`Overwrite `,
    /// the path, `? (y/n)`). The question is drawn in `help_text` and the
    /// subject in `foreground`; a subject too long for the row is cut at its
    /// start with `…` so that the end of the question stays visible.
    pub fn confirm(before: &str, subject: &str, after: &str, theme: Theme) -> Self {
        let mut prompt = Self::new(before, "", PromptMode::YesNo, theme);
        prompt.question = Some(Question {
            before: before.to_owned(),
            subject: subject.to_owned(),
            between: String::new(),
            second: String::new(),
            after: after.to_owned(),
        });
        prompt
    }

    /// Like [`Prompt::confirm`], with two subjects: `before` + `subject` +
    /// `between` + `second` + `after` (`Resume `, the id, ` in `, the
    /// harness, `? (y/n)`). Only the first subject is cut when the row is too
    /// narrow.
    pub fn confirm_two(
        before: &str,
        subject: &str,
        between: &str,
        second: &str,
        after: &str,
        theme: Theme,
    ) -> Self {
        let mut prompt = Self::confirm(before, subject, after, theme);
        if let Some(question) = &mut prompt.question {
            question.between = between.to_owned();
            question.second = second.to_owned();
        }
        prompt
    }

    /// Paints the row with `fill` instead of `surface` before drawing (the
    /// browser's prompt will sit on another background than the selector's).
    #[must_use]
    pub fn with_fill(mut self, fill: Role) -> Self {
        self.fill = fill;
        self
    }

    /// Keeps `columns` blank columns free on both sides of the row, like
    /// the browser's help line.
    #[must_use]
    pub fn inset(mut self, columns: u16) -> Self {
        self.inset = columns;
        self
    }

    /// The text typed so far.
    pub fn text(&self) -> &str {
        self.input.value()
    }

    /// The cursor's position in characters from the start of the text.
    pub fn cursor(&self) -> usize {
        self.input.cursor()
    }

    /// Applies `request` and returns the message the change calls for.
    fn edit(&mut self, request: InputRequest) -> Option<Msg> {
        let changed = self.input.handle(request)?;
        if changed.value {
            Some(Msg::PromptChanged(self.text().to_owned()))
        } else if changed.cursor {
            Some(Msg::PromptMoved)
        } else {
            None
        }
    }

    fn on_text_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Enter => Some(Msg::PromptSubmit(self.text().to_owned())),
            Key::Esc => Some(Msg::PromptCancel),
            Key::Up => Some(Msg::PromptNav(Direction::Up)),
            Key::Down => Some(Msg::PromptNav(Direction::Down)),
            Key::Left => self.edit(InputRequest::GoToPrevChar),
            Key::Right => self.edit(InputRequest::GoToNextChar),
            Key::Backspace => self.edit(InputRequest::DeletePrevChar),
            Key::Char(c) if is_typed(key.modifiers) => self.edit(InputRequest::InsertChar(c)),
            _ => None,
        }
    }

    fn on_yes_no_key(key: &KeyEvent) -> Msg {
        match key.code {
            Key::Char('y' | 'Y') if is_typed(key.modifiers) => Msg::PromptSubmit("y".to_owned()),
            _ => Msg::PromptCancel,
        }
    }

    fn paint_fill(&self, frame: &mut Frame, area: Rect) {
        if !self.theme.transparent_background() {
            let style = Style::new().bg(self.theme.role(self.fill));
            frame.render_widget(Block::new().style(style), area);
        }
    }

    /// Draws the question of a [`Prompt::confirm`] on `row`.
    fn draw_question(&self, frame: &mut Frame, row: Rect, question: &Question) {
        use unicode_width::UnicodeWidthStr;
        let text_style = self.theme.style(Token::HelpText);
        let subject_style = Style::new().fg(self.theme.role(Role::Foreground));
        let fixed = question.before.width()
            + question.between.width()
            + question.second.width()
            + question.after.width();
        let room = usize::from(row.width).saturating_sub(fixed);
        let subject = truncate_start(&question.subject, room);
        let mut spans = vec![
            Span::styled(question.before.clone(), text_style),
            Span::styled(subject, subject_style),
        ];
        if !question.second.is_empty() {
            spans.push(Span::styled(question.between.clone(), text_style));
            spans.push(Span::styled(question.second.clone(), subject_style));
        }
        spans.push(Span::styled(question.after.clone(), text_style));
        let line = Line::from(spans);
        frame.render_widget(Paragraph::new(line), row);
    }

    /// The part of the text that is visible in `width` columns, and the
    /// column of the cursor in it.
    ///
    /// The text scrolls, by whole characters, the least that shows the
    /// whole cell under the cursor: the character there, which may be two
    /// columns wide, or the blank cell after the end of the text.
    fn visible_text(&self, width: usize) -> (String, usize) {
        let cursor_index = self.input.cursor();
        let widths: Vec<usize> = self
            .text()
            .chars()
            .map(|c| c.width().unwrap_or(0))
            .collect();
        let cursor_column: usize = widths.iter().take(cursor_index).sum();
        let under = widths.get(cursor_index).copied().unwrap_or(1).max(1);
        let wanted = (cursor_column + under)
            .saturating_sub(width)
            .min(cursor_column);
        let mut skipped = 0;
        let mut shown = 0;
        let mut text = String::new();
        for (c, w) in self.text().chars().zip(widths) {
            if skipped < wanted {
                skipped += w;
                continue;
            }
            if shown + w > width {
                break;
            }
            shown += w;
            text.push(c);
        }
        (text, cursor_column.saturating_sub(skipped))
    }
}

/// The visible part of a prompt's text in `foreground`, and the cell under
/// the cursor with the `cursor` colour as its background: the character at
/// column `cursor`, both columns of a wide one, or the blank cell after the
/// end of the text. A cursor past the area's width draws no cursor cell.
#[derive(Debug, Clone, Copy)]
struct PromptText<'a> {
    text: &'a str,
    /// The column of the cursor in `text`.
    cursor: usize,
    theme: &'a Theme,
}

impl PromptText<'_> {
    /// The width of the cell under the cursor: the character starting at
    /// the cursor's column, or 1 for the blank cell after the text.
    fn cursor_width(&self) -> usize {
        self.text
            .chars()
            .scan(0, |col, c| {
                let start = *col;
                *col += c.width().unwrap_or(0);
                Some((start, c.width().unwrap_or(0)))
            })
            .find(|&(start, _)| start == self.cursor)
            .map_or(1, |(_, w)| w.max(1))
    }
}

impl Widget for PromptText<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let foreground = self.theme.role(Role::Foreground);
        let style = Style::new().fg(foreground);
        Paragraph::new(Span::styled(self.text, style)).render(area, buf);
        let cursor_style = style.bg(self.theme.role(Role::Cursor));
        for offset in 0..self.cursor_width() {
            let x = self.cursor + offset;
            if x < usize::from(area.width) {
                // `x` is below `area.width`, a `u16`.
                buf[(area.x + x as u16, area.y)].set_style(cursor_style);
            }
        }
    }
}

/// Whether a key with `modifiers` types its character: plain, shifted, or
/// with `AltGr`, which Windows reports as `Ctrl` + `Alt` (the `@`, `{` or
/// `\` of many layouts). `Ctrl` or `Alt` alone do not type.
fn is_typed(modifiers: KeyModifiers) -> bool {
    (modifiers - KeyModifiers::SHIFT).is_empty()
        || modifiers.contains(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

impl Component for Prompt {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        self.paint_fill(frame, area);
        let inset = self.inset.min(area.width / 2);
        let row = Rect::new(area.x + inset, area.bottom() - 1, area.width - 2 * inset, 1);
        if let Some(question) = &self.question {
            self.draw_question(frame, row, question);
            return;
        }
        let label_width = u16::try_from(unicode_width::UnicodeWidthStr::width(self.label.as_str()))
            .unwrap_or(u16::MAX)
            .min(row.width);
        let label = Span::styled(self.label.clone(), self.theme.style(Token::HelpKey));
        let label_area = Rect::new(row.x, row.y, label_width, 1);
        frame.render_widget(Paragraph::new(Line::from(label)), label_area);
        let rest = Rect::new(
            row.x + label_width,
            row.y,
            row.width - label_width,
            row.height,
        );
        if rest.is_empty() || self.mode == PromptMode::YesNo {
            return;
        }
        let (text, cursor) = self.visible_text(usize::from(rest.width));
        let text = PromptText {
            text: &text,
            cursor,
            theme: &self.theme,
        };
        frame.render_widget(text, rest);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            Attribute::Focus => Some(QueryResult::Owned(AttrValue::Flag(self.focused))),
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            _ => None,
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        match attr {
            Attribute::Focus => {
                if let AttrValue::Flag(focused) = value {
                    self.focused = focused;
                }
            }
            THEME_ATTR => {
                if let Some(theme) = theme_from_value(&value) {
                    self.theme = theme;
                }
            }
            _ => {}
        }
    }

    fn state(&self) -> State {
        State::Single(StateValue::String(self.text().to_owned()))
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for Prompt {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) if KeySpec::from_event(key).is_ctrl_c() => Some(Msg::Quit),
            Event::Keyboard(key) => match self.mode {
                PromptMode::Text => self.on_text_key(key),
                PromptMode::YesNo => Some(Self::on_yes_no_key(key)),
            },
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;
    use ratatui::style::Color;

    fn prompt(text: &str) -> Prompt {
        Prompt::new("/", text, PromptMode::Text, builtin::default_theme())
    }

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn ch(c: char) -> Event<UserEvent> {
        key(Key::Char(c))
    }

    fn changed(text: &str) -> Option<Msg> {
        Some(Msg::PromptChanged(text.to_owned()))
    }

    fn draw(prompt: &mut Prompt, width: u16) -> ratatui::buffer::Buffer {
        render(width, 1, |frame| prompt.view(frame, frame.area()))
    }

    // -- editing

    #[test]
    fn typing_inserts_characters_and_reports_the_text() {
        let mut p = prompt("");
        assert_eq!(p.on(&ch('a')), changed("a"));
        assert_eq!(p.on(&ch('b')), changed("ab"));
        assert_eq!(p.on(&ch('c')), changed("abc"));
        assert_eq!((p.text(), p.cursor()), ("abc", 3));
    }

    #[test]
    fn backspace_deletes_the_character_before_the_cursor() {
        let mut p = prompt("abc");
        assert_eq!(p.on(&key(Key::Backspace)), changed("ab"));
        assert_eq!((p.text(), p.cursor()), ("ab", 2));
    }

    #[test]
    fn backspace_on_an_empty_text_changes_nothing() {
        let mut p = prompt("");
        assert_eq!(p.on(&key(Key::Backspace)), None);
    }

    #[test]
    fn left_moves_the_cursor_and_typing_inserts_there() {
        let mut p = prompt("abc");
        assert_eq!(p.on(&key(Key::Left)), Some(Msg::PromptMoved));
        assert_eq!((p.text(), p.cursor()), ("abc", 2));
        assert_eq!(p.on(&ch('X')), changed("abXc"));
        assert_eq!(p.cursor(), 3);
        assert_eq!(p.on(&key(Key::Right)), Some(Msg::PromptMoved));
        assert_eq!(p.cursor(), 4);
        assert_eq!(p.on(&key(Key::Right)), None, "already at the end");
    }

    #[test]
    fn left_at_the_start_moves_nothing() {
        let mut p = prompt("");
        assert_eq!(p.on(&key(Key::Left)), None);
        let mut p = prompt("ab");
        p.on(&key(Key::Left));
        p.on(&key(Key::Left));
        assert_eq!(p.cursor(), 0);
        assert_eq!(p.on(&key(Key::Left)), None);
    }

    #[test]
    fn backspace_after_left_deletes_before_the_cursor() {
        let mut p = prompt("abc");
        p.on(&key(Key::Left));
        assert_eq!(p.on(&key(Key::Backspace)), changed("ac"));
        assert_eq!(p.cursor(), 1);
    }

    #[test]
    fn q_t_digits_and_h_are_text() {
        let mut p = prompt("");
        for c in ['q', 't', '1', 'h', 'l', 'j', '?', '/'] {
            assert!(matches!(p.on(&ch(c)), Some(Msg::PromptChanged(_))), "{c}");
        }
        assert_eq!(p.text(), "qt1hlj?/");
    }

    #[test]
    fn shifted_characters_are_text_and_ctrl_or_alt_ones_are_not() {
        let mut p = prompt("");
        let shifted = Event::Keyboard(KeyEvent::new(Key::Char('Q'), KeyModifiers::SHIFT));
        assert_eq!(p.on(&shifted), changed("Q"));
        for modifiers in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            let ev = Event::Keyboard(KeyEvent::new(Key::Char('q'), modifiers));
            assert_eq!(p.on(&ev), None, "{modifiers:?}");
        }
        assert_eq!(p.text(), "Q");
    }

    #[test]
    fn wide_characters_move_and_delete_as_one() {
        let mut p = prompt("");
        p.on(&ch('日'));
        p.on(&ch('本'));
        assert_eq!(p.on(&key(Key::Left)), Some(Msg::PromptMoved));
        assert_eq!(p.cursor(), 1);
        assert_eq!(p.on(&key(Key::Backspace)), changed("本"));
    }

    #[test]
    fn altgr_characters_are_text() {
        // Windows reports AltGr as Ctrl + Alt.
        let mut p = prompt("");
        let altgr = KeyModifiers::CONTROL | KeyModifiers::ALT;
        let at = Event::Keyboard(KeyEvent::new(Key::Char('@'), altgr));
        assert_eq!(p.on(&at), changed("@"));
        let brace = Event::Keyboard(KeyEvent::new(Key::Char('{'), altgr | KeyModifiers::SHIFT));
        assert_eq!(p.on(&brace), changed("@{"));
    }

    // -- the keys that keep their meaning

    #[test]
    fn enter_submits_the_text() {
        let mut p = prompt("abc");
        assert_eq!(
            p.on(&key(Key::Enter)),
            Some(Msg::PromptSubmit("abc".to_owned()))
        );
        assert_eq!(p.text(), "abc", "the owner decides what happens next");
    }

    #[test]
    fn esc_cancels() {
        assert_eq!(prompt("abc").on(&key(Key::Esc)), Some(Msg::PromptCancel));
    }

    #[test]
    fn up_and_down_navigate() {
        let mut p = prompt("abc");
        assert_eq!(
            p.on(&key(Key::Up)),
            Some(Msg::PromptNav(Direction::Up)),
            "up"
        );
        assert_eq!(
            p.on(&key(Key::Down)),
            Some(Msg::PromptNav(Direction::Down)),
            "down"
        );
        assert_eq!(p.text(), "abc");
    }

    #[test]
    fn ctrl_c_quits_in_both_modes() {
        let ctrl_c = Event::Keyboard(KeyEvent::new(Key::Char('c'), KeyModifiers::CONTROL));
        assert_eq!(prompt("abc").on(&ctrl_c), Some(Msg::Quit));
        let mut yes_no = Prompt::new("Sure? ", "", PromptMode::YesNo, builtin::default_theme());
        assert_eq!(yes_no.on(&ctrl_c), Some(Msg::Quit));
    }

    #[test]
    fn other_keys_and_events_do_nothing() {
        let mut p = prompt("abc");
        for ev in [
            key(Key::Tab),
            key(Key::BackTab),
            key(Key::PageUp),
            key(Key::Home),
            key(Key::End),
            key(Key::Delete),
            Event::Tick,
            Event::WindowResize(10, 10),
            Event::FocusLost,
        ] {
            assert_eq!(p.on(&ev), None, "{ev:?}");
        }
    }

    // -- yes/no

    fn yes_no() -> Prompt {
        Prompt::new(
            "Overwrite? (y/n)",
            "",
            PromptMode::YesNo,
            builtin::default_theme(),
        )
    }

    #[test]
    fn y_submits_and_n_cancels() {
        let mut p = yes_no();
        assert_eq!(p.on(&ch('y')), Some(Msg::PromptSubmit("y".to_owned())));
        assert_eq!(p.on(&ch('Y')), Some(Msg::PromptSubmit("y".to_owned())));
        assert_eq!(p.on(&ch('n')), Some(Msg::PromptCancel));
    }

    #[test]
    fn every_other_key_cancels_a_yes_no_prompt() {
        let mut p = yes_no();
        for code in [Key::Esc, Key::Enter, Key::Char('x'), Key::Tab, Key::Up] {
            assert_eq!(p.on(&key(code)), Some(Msg::PromptCancel), "{code:?}");
        }
        let ctrl_y = Event::Keyboard(KeyEvent::new(Key::Char('y'), KeyModifiers::CONTROL));
        assert_eq!(p.on(&ctrl_y), Some(Msg::PromptCancel));
        assert_eq!(p.text(), "", "nothing is typed");
        assert_eq!(p.on(&Event::Tick), None);
    }

    #[test]
    fn a_yes_no_prompt_shows_only_its_question() {
        let text = buffer_to_text(&draw(&mut yes_no(), 30));
        assert_eq!(text, "Overwrite? (y/n)              \n");
    }

    // -- focus and state

    #[test]
    fn focus_and_text_can_be_queried() {
        let mut p = prompt("abc");
        p.attr(Attribute::Focus, AttrValue::Flag(true));
        assert!(matches!(
            p.query(Attribute::Focus).map(QueryResult::into_attr),
            Some(AttrValue::Flag(true))
        ));
        assert_eq!(p.state(), State::Single(StateValue::String("abc".into())));
    }

    // -- drawing

    #[test]
    fn the_label_and_the_text_are_drawn_with_a_cursor_cell_after_them() {
        let theme = builtin::default_theme();
        let mut p = prompt("abc");
        let buffer = draw(&mut p, 20);
        assert_eq!(buffer_to_text(&buffer), "/abc                \n");
        assert_eq!(buffer[(0, 0)].fg, theme.role(Role::Primary));
        assert_eq!(buffer[(1, 0)].fg, theme.role(Role::Foreground));
        assert_eq!(buffer[(4, 0)].bg, theme.role(Role::Cursor));
        assert_ne!(buffer[(3, 0)].bg, theme.role(Role::Cursor));
        assert_ne!(buffer[(5, 0)].bg, theme.role(Role::Cursor));
    }

    #[test]
    fn the_cursor_cell_follows_the_cursor_inside_the_text() {
        let theme = builtin::default_theme();
        let mut p = prompt("abc");
        p.on(&key(Key::Left));
        let buffer = draw(&mut p, 20);
        assert_eq!(buffer[(3, 0)].bg, theme.role(Role::Cursor));
        assert_eq!(buffer[(3, 0)].symbol(), "c");
        assert_ne!(buffer[(4, 0)].bg, theme.role(Role::Cursor));
    }

    #[test]
    fn an_inset_prompt_starts_that_many_columns_in_and_keeps_the_fill_across() {
        let theme = builtin::default_theme();
        let mut p = prompt("abc").inset(1).with_fill(Role::Background);
        let buffer = draw(&mut p, 20);
        assert_eq!(buffer_to_text(&buffer), " /abc               \n");
        assert_eq!(buffer[(1, 0)].fg, theme.role(Role::Primary));
        assert_eq!(buffer[(5, 0)].bg, theme.role(Role::Cursor));
        assert_eq!(buffer[(0, 0)].bg, theme.role(Role::Background));
        assert_eq!(buffer[(19, 0)].bg, theme.role(Role::Background));
    }

    #[test]
    fn a_long_text_scrolls_inside_the_inset() {
        let mut p = prompt(&"x".repeat(40)).inset(1);
        let buffer = draw(&mut p, 12);
        let text = buffer_to_text(&buffer);
        assert!(text.starts_with(" /xxx"), "{text:?}");
        assert!(text.trim_end().ends_with("xxx"), "{text:?}");
        assert_eq!(text.trim_end_matches('\n').chars().count(), 12);
    }

    #[test]
    fn an_empty_prompt_is_the_label_and_the_cursor() {
        let theme = builtin::default_theme();
        let buffer = draw(&mut prompt(""), 20);
        assert_eq!(buffer[(1, 0)].bg, theme.role(Role::Cursor));
    }

    #[test]
    fn the_row_is_painted_with_the_fill_unless_the_theme_is_transparent() {
        let theme = builtin::default_theme();
        let buffer = draw(&mut prompt("a"), 20);
        assert_eq!(buffer[(15, 0)].bg, theme.role(Role::Surface));
        let mut p = prompt("a").with_fill(Role::Background);
        assert_eq!(draw(&mut p, 20)[(15, 0)].bg, theme.role(Role::Background));
    }

    const LONG: &str = "the quick brown fox jumps over the lazy dog";

    #[test]
    fn long_text_scrolls_to_keep_the_cursor_visible() {
        let mut p = prompt(LONG);
        let text = buffer_to_text(&draw(&mut p, 30));
        // 29 columns after the label; the last one is the cursor cell.
        assert_eq!(text, "/ fox jumps over the lazy dog \n");
    }

    #[test]
    fn the_cursor_stays_in_view_when_moving_left_through_long_text() {
        let theme = builtin::default_theme();
        let mut p = prompt(LONG);
        for _ in 0..LONG.len() {
            p.on(&key(Key::Left));
            let buffer = draw(&mut p, 30);
            let cursors = (0..30)
                .filter(|&x| buffer[(x, 0)].bg == theme.role(Role::Cursor))
                .count();
            assert_eq!(cursors, 1, "cursor at {}", p.cursor());
        }
        assert_eq!(p.cursor(), 0);
        assert!(buffer_to_text(&draw(&mut p, 30)).starts_with("/the quick"));
    }

    #[test]
    fn scrolled_text_snapshot_width_30() {
        let mut p = prompt(LONG);
        insta::assert_snapshot!(buffer_to_styled(&draw(&mut p, 30)));
    }

    #[test]
    fn scrolled_text_with_the_cursor_inside_snapshot_width_30() {
        let mut p = prompt(LONG);
        for _ in 0..20 {
            p.on(&key(Key::Left));
        }
        insta::assert_snapshot!(buffer_to_styled(&draw(&mut p, 30)));
    }

    const WIDE: &str = "日本語日本語日本語日本語日本語日本語日本語日本語";

    #[test]
    fn the_wide_character_under_the_cursor_is_drawn_with_the_cursor_background() {
        let theme = builtin::default_theme();
        let chars: Vec<char> = WIDE.chars().collect();
        for lefts in 0..=chars.len() {
            let mut p = prompt(WIDE);
            for _ in 0..lefts {
                p.on(&key(Key::Left));
            }
            let buffer = draw(&mut p, 30);
            let cursor_cells: Vec<u16> = (0..30)
                .filter(|&x| buffer[(x, 0)].bg == theme.role(Role::Cursor))
                .collect();
            // A wide glyph and its second cell are one cell for the buffer.
            assert_eq!(cursor_cells.len(), 1, "{lefts} lefts: {cursor_cells:?}");
            if lefts > 0 {
                let glyph = chars[chars.len() - lefts].to_string();
                assert_eq!(
                    buffer[(cursor_cells[0], 0)].symbol(),
                    glyph,
                    "{lefts} lefts"
                );
            }
        }
    }

    #[test]
    fn wide_text_scrolls_snapshot_width_30() {
        let mut p = prompt(WIDE);
        p.on(&key(Key::Left));
        insta::assert_snapshot!(buffer_to_styled(&draw(&mut p, 30)));
    }

    // -- the text widget

    /// `text` drawn by [`PromptText`] with the cursor at `cursor`, in a
    /// `width` x 1 area at column 1 of a buffer one column wider. The widget
    /// draws straight into the buffer: a terminal would not pass on the
    /// cell a wide character covers.
    fn draw_text(text: &str, cursor: usize, width: u16) -> Buffer {
        let theme = builtin::default_theme();
        let widget = PromptText {
            text,
            cursor,
            theme: &theme,
        };
        let mut buffer = Buffer::empty(Rect::new(0, 0, width + 1, 1));
        widget.render(Rect::new(1, 0, width, 1), &mut buffer);
        buffer
    }

    #[test]
    fn the_text_widget_draws_the_text_in_foreground_and_the_cursor_cell() {
        let theme = builtin::default_theme();
        let buffer = draw_text("abc", 1, 6);
        assert_eq!(buffer_to_text(&buffer), " abc   \n");
        let foreground = theme.role(Role::Foreground);
        let cursor = theme.role(Role::Cursor);
        for x in [1, 3] {
            assert_eq!(buffer[(x, 0)].fg, foreground, "{x}");
            assert_eq!(buffer[(x, 0)].bg, Color::Reset, "{x}");
        }
        assert_eq!(buffer[(2, 0)].symbol(), "b");
        assert_eq!((buffer[(2, 0)].fg, buffer[(2, 0)].bg), (foreground, cursor));
        assert_eq!(buffer[(0, 0)].bg, Color::Reset, "left of the area");
    }

    #[test]
    fn the_text_widget_marks_both_columns_of_a_wide_character() {
        let cursor = builtin::default_theme().role(Role::Cursor);
        let buffer = draw_text("a日b", 1, 6);
        let backgrounds: Vec<Color> = (1..6).map(|x| buffer[(x, 0)].bg).collect();
        assert_eq!(
            backgrounds,
            [Color::Reset, cursor, cursor, Color::Reset, Color::Reset]
        );
    }

    #[test]
    fn the_text_widget_marks_the_blank_cell_after_the_text() {
        let cursor = builtin::default_theme().role(Role::Cursor);
        let buffer = draw_text("ab", 2, 4);
        assert_eq!(buffer[(3, 0)].symbol(), " ");
        assert_eq!(buffer[(3, 0)].bg, cursor);
        assert_eq!(buffer[(4, 0)].bg, Color::Reset);
    }

    #[test]
    fn the_text_widget_draws_no_cursor_cell_outside_its_area() {
        let buffer = draw_text("abc", 3, 3);
        for x in 0..4 {
            assert_eq!(buffer[(x, 0)].bg, Color::Reset, "{x}");
        }
    }

    // -- confirm

    fn confirm(subject: &str) -> Prompt {
        Prompt::confirm("Overwrite ", subject, "? (y/n)", builtin::default_theme())
    }

    #[test]
    fn a_confirm_prompt_asks_its_question_in_help_text_with_the_subject_in_foreground() {
        let theme = builtin::default_theme();
        let buffer = draw(&mut confirm("./notes.txt"), 40);
        assert_eq!(
            buffer_to_text(&buffer),
            "Overwrite ./notes.txt? (y/n)            \n"
        );
        let fg = |x| buffer[(x, 0)].fg;
        let help_text = theme.style(Token::HelpText).fg.unwrap();
        let foreground = theme.role(Role::Foreground);
        assert_ne!(help_text, foreground, "the colours must tell them apart");
        assert!((0..10).all(|x| fg(x) == help_text), "Overwrite ");
        assert!((10..21).all(|x| fg(x) == foreground), "the path");
        assert!((21..28).all(|x| fg(x) == help_text), "? (y/n)");
        // No cursor: nothing is typed.
        assert!((0..40).all(|x| buffer[(x, 0)].bg != theme.role(Role::Cursor)));
    }

    #[test]
    fn a_two_subject_confirm_prompt_has_both_subjects_in_foreground() {
        let theme = builtin::default_theme();
        let mut p = Prompt::confirm_two(
            "Resume ",
            "85a86657",
            " in ",
            "Claude",
            "? (y/n)",
            builtin::default_theme(),
        );
        let buffer = draw(&mut p, 40);
        assert_eq!(
            buffer_to_text(&buffer),
            "Resume 85a86657 in Claude? (y/n)        \n"
        );
        let fg = |x| buffer[(x, 0)].fg;
        let help_text = theme.style(Token::HelpText).fg.unwrap();
        let foreground = theme.role(Role::Foreground);
        assert!((0..7).all(|x| fg(x) == help_text), "Resume ");
        assert!((7..15).all(|x| fg(x) == foreground), "the id");
        assert!((15..19).all(|x| fg(x) == help_text), " in ");
        assert!((19..25).all(|x| fg(x) == foreground), "the harness");
        assert!((25..32).all(|x| fg(x) == help_text), "? (y/n)");
        // It answers like any y/n prompt.
        assert_eq!(p.on(&ch('y')), Some(Msg::PromptSubmit("y".to_owned())));
        assert_eq!(p.on(&ch('n')), Some(Msg::PromptCancel));
    }

    #[test]
    fn a_confirm_prompt_answers_like_a_yes_no_prompt() {
        let mut p = confirm("./notes.txt");
        assert_eq!(p.on(&ch('y')), Some(Msg::PromptSubmit("y".to_owned())));
        assert_eq!(p.on(&ch('n')), Some(Msg::PromptCancel));
        assert_eq!(p.on(&key(Key::Esc)), Some(Msg::PromptCancel));
        assert_eq!(p.on(&key(Key::Enter)), Some(Msg::PromptCancel));
    }

    #[test]
    fn a_subject_too_long_for_the_row_is_cut_at_its_start() {
        let text = buffer_to_text(&draw(&mut confirm("/very/long/directory/notes.txt"), 30));
        assert_eq!(text, "Overwrite …ry/notes.txt? (y/n)\n");
    }

    #[test]
    fn a_tiny_area_does_not_panic() {
        for width in 0..4 {
            let mut p = prompt(LONG);
            draw(&mut p, width);
        }
        let mut p = prompt(LONG);
        let buffer = render(5, 1, |frame| p.view(frame, Rect::new(0, 0, 1, 1)));
        assert_eq!(buffer[(0, 0)].symbol(), "/");
        assert_ne!(buffer[(0, 0)].bg, Color::Reset);
    }
}
