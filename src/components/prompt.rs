//! The help-line prompt: search, file paths and `y/n` questions (SPEC §1.3,
//! §4.2, §4.3, §6).
//!
//! While a prompt is active it has the focus (input mode): every printable
//! key goes into the text. Only `Enter`, `Esc`, `Backspace`, the arrow keys
//! and `Ctrl-c` keep a meaning, so none of the browser's or selector's keys
//! conflict with typing.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use tui_input::{Input, InputRequest};
use tuirealm::command::{Cmd, CmdResult, Direction};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};
use unicode_width::UnicodeWidthChar;

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
    focused: bool,
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
            focused: false,
        }
    }

    /// Paints the row with `fill` instead of `surface` before drawing.
    #[must_use]
    pub fn with_fill(mut self, fill: Role) -> Self {
        self.fill = fill;
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
            Key::Home => self.edit(InputRequest::GoToStart),
            Key::End => self.edit(InputRequest::GoToEnd),
            Key::Backspace => self.edit(InputRequest::DeletePrevChar),
            Key::Delete => self.edit(InputRequest::DeleteNextChar),
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

    /// Inserts pasted text (on one line) and reports the change.
    fn paste(&mut self, text: &str) -> Option<Msg> {
        let mut changed = false;
        for c in text.chars().filter(|c| !c.is_control()) {
            changed |= self.input.handle(InputRequest::InsertChar(c)).is_some();
        }
        changed.then(|| Msg::PromptChanged(self.text().to_owned()))
    }

    fn paint_fill(&self, frame: &mut Frame, area: Rect) {
        if !self.theme.transparent_background() {
            let style = Style::new().bg(self.theme.role(self.fill));
            frame.render_widget(Block::new().style(style), area);
        }
    }

    /// The part of the text that is visible in `width` columns, and the
    /// column of the cursor in it. The last column is kept for the cursor
    /// when it is after the end of the text.
    fn visible_text(&self, width: usize) -> (String, usize) {
        let scroll = self.input.visual_scroll(width.saturating_sub(1));
        let mut skipped = 0;
        let mut shown = 0;
        let mut text = String::new();
        for c in self.text().chars() {
            let w = c.width().unwrap_or(0);
            if skipped < scroll {
                skipped += w;
                continue;
            }
            if shown + w > width {
                break;
            }
            shown += w;
            text.push(c);
        }
        (text, self.input.visual_cursor().saturating_sub(skipped))
    }
}

/// Whether a key with `modifiers` types its character: plain or shifted.
fn is_typed(modifiers: KeyModifiers) -> bool {
    modifiers == KeyModifiers::NONE || modifiers == KeyModifiers::SHIFT
}

impl Component for Prompt {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        self.paint_fill(frame, area);
        let row = Rect::new(area.x, area.bottom() - 1, area.width, 1);
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
        let foreground = self.theme.role(Role::Foreground);
        let style = Style::new().fg(foreground);
        frame.render_widget(Paragraph::new(Span::styled(text.clone(), style)), rest);
        // The cell under the cursor: the character there, or the blank cell
        // after the text.
        let under = text
            .chars()
            .scan(0, |col, c| {
                let start = *col;
                *col += c.width().unwrap_or(0);
                Some((start, c.width().unwrap_or(0)))
            })
            .find(|&(start, _)| start == cursor)
            .map_or(1, |(_, w)| w.max(1));
        let cursor_bg = self.theme.role(Role::Cursor);
        for offset in 0..under {
            let x = cursor + offset;
            if x < usize::from(rest.width) {
                // `x` is below `rest.width`, a `u16`.
                let cell = &mut frame.buffer_mut()[(rest.x + x as u16, rest.y)];
                cell.set_style(Style::new().fg(foreground).bg(cursor_bg));
            }
        }
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
            Event::Paste(text) if self.mode == PromptMode::Text => self.paste(text),
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
        p.on(&key(Key::Home));
        assert_eq!(p.cursor(), 0);
        assert_eq!(p.on(&key(Key::Left)), None);
        assert_eq!(p.on(&key(Key::End)), Some(Msg::PromptMoved));
        assert_eq!(p.cursor(), 2);
    }

    #[test]
    fn backspace_after_left_deletes_before_the_cursor() {
        let mut p = prompt("abc");
        p.on(&key(Key::Left));
        assert_eq!(p.on(&key(Key::Backspace)), changed("ac"));
        assert_eq!(p.cursor(), 1);
        assert_eq!(p.on(&key(Key::Delete)), changed("a"));
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
    fn pasted_text_is_inserted_on_one_line() {
        let mut p = prompt("a");
        assert_eq!(p.on(&Event::Paste("b\nc".to_owned())), changed("abc"));
        assert_eq!(p.on(&Event::Paste("\n".to_owned())), None);
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
