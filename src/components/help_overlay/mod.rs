//! The help overlay (SPEC §4.3): the keybindings grouped by purpose, generated
//! from the active keymap, in a centred rounded overlay that scrolls when it
//! is taller than the screen.

mod content;

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};

pub use content::{HelpContent, HelpContext, HelpGroup, HelpRow, MIN_WIDTH, SHIFT_NOTE};

use super::overlay_frame::{FrameKey, OverlayFrame};
pub use super::overlay_frame::{INTERIOR_ATTR, interior_value};
use super::{THEME_ATTR, payload_from_value, payload_value, theme_from_value, theme_value};
use crate::app::{Msg, RoutedMouse, UserEvent};
use crate::keymap::{Action, Context, Keymap};
use crate::theme::Theme;

/// The attribute that tells the global keys where the help is opened from (a
/// [`HelpContext`] payload, see [`context_value`]).
pub const HELP_CONTEXT_ATTR: Attribute = Attribute::Custom("help_context");

/// The attribute value that sets where the help is opened from.
pub fn context_value(context: HelpContext) -> AttrValue {
    payload_value(context)
}

/// The context in an attribute value made by [`context_value`].
pub fn context_from_value(value: &AttrValue) -> Option<HelpContext> {
    payload_from_value(value)
}

/// The help overlay. It takes the focus; `Esc` or the global help key closes
/// it, the up and down keys scroll a row and the page keys a page.
///
/// `view` gets the overlay's rect (see `App`'s layout) as its area. The
/// frame (border, fill, `more` indicators and scrolling) is the
/// [`OverlayFrame`] that the details overlay uses too.
#[derive(Debug, Clone)]
pub struct HelpOverlay {
    content: HelpContent,
    frame: OverlayFrame,
}

impl HelpOverlay {
    /// An overlay that lists `content`, resolving keys with `keymap`.
    pub fn new(content: HelpContent, theme: Theme, keymap: Keymap) -> Self {
        Self {
            content,
            frame: OverlayFrame::new("Keybindings", theme, keymap),
        }
    }
}

impl Component for HelpOverlay {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.frame.theme().clone();
        self.frame.draw(frame, area, |_| self.content.lines(&theme));
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        if attr == THEME_ATTR {
            Some(QueryResult::Owned(theme_value(self.frame.theme())))
        } else if attr == INTERIOR_ATTR {
            self.frame
                .interior()
                .map(|size| QueryResult::Owned(interior_value(size)))
        } else {
            None
        }
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == THEME_ATTR {
            if let Some(theme) = theme_from_value(&value) {
                self.frame.set_theme(theme);
            }
        } else if attr == INTERIOR_ATTR
            && let Some(size) = payload_from_value::<Size>(&value)
        {
            self.frame.set_interior(size, self.content.row_count());
        }
    }

    /// The first row shown.
    fn state(&self) -> State {
        State::Single(StateValue::Usize(self.frame.offset()))
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl HelpOverlay {
    /// The wheel scrolls [`WHEEL_ROWS`](super::overlay_frame::WHEEL_ROWS) rows; clicks do nothing (the `App`
    /// closes the overlay for one outside it).
    fn on_mouse(&mut self, mouse: &RoutedMouse) -> Option<Msg> {
        self.frame
            .on_mouse(mouse, self.content.row_count())
            .then_some(Msg::HelpScrolled)
    }
}

impl AppComponent<Msg, UserEvent> for HelpOverlay {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let key = match ev {
            Event::Keyboard(key) => key,
            // Only what the `App` routed here, in the overlay's coordinates.
            Event::User(UserEvent::Mouse(mouse)) => return self.on_mouse(mouse),
            _ => return None,
        };
        match self.frame.on_key(key, self.content.row_count()) {
            FrameKey::Close => Some(Msg::CloseHelp),
            FrameKey::Scrolled => Some(Msg::HelpScrolled),
            FrameKey::Unmoved => None,
            FrameKey::Unbound => (self.frame.keymap().resolve(Context::Global, key)
                == Some(Action::Help))
            .then_some(Msg::CloseHelp),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::overlay_frame::WHEEL_ROWS;
    use super::*;
    use crate::model::HarnessId;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::{Role, Token, builtin};
    use ratatui::widgets::Paragraph;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers, MouseEventKind};

    /// The rows of the browser's help with the mouse note.
    const ROWS: usize = 51;

    fn overlay_with(keymap: Keymap, mouse: bool) -> HelpOverlay {
        let content = HelpContent::new(&keymap, HelpContext::Browser, &[HarnessId::Claude], mouse);
        HelpOverlay::new(content, builtin::default_theme(), keymap)
    }

    fn overlay() -> HelpOverlay {
        overlay_with(Keymap::default(), true)
    }

    /// An overlay whose interior has `rows` rows.
    fn with_interior(rows: u16) -> HelpOverlay {
        let mut overlay = overlay();
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, rows)));
        overlay
    }

    fn press(overlay: &mut HelpOverlay, code: Key) -> Option<Msg> {
        let event = Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE));
        overlay.on(&event)
    }

    fn offset(overlay: &HelpOverlay) -> usize {
        match overlay.state() {
            State::Single(StateValue::Usize(offset)) => offset,
            other => panic!("{other:?}"),
        }
    }

    /// The overlay drawn the way the app places it: `rows` rows inside, the
    /// widest entry plus eight columns, on a 100 column screen.
    fn draw(overlay: &mut HelpOverlay, rows: u16) -> ratatui::buffer::Buffer {
        let area = Rect::new(10, 1, 72, rows + 2);
        render(100, rows + 4, |frame| overlay.view(frame, area))
    }

    fn text_of(overlay: &mut HelpOverlay, rows: u16) -> Vec<String> {
        let text = buffer_to_text(&draw(overlay, rows));
        text.lines().map(str::to_owned).collect()
    }

    #[test]
    fn the_down_keys_scroll_one_row_and_clamp_at_the_end() {
        let mut overlay = with_interior(20);
        let last = ROWS - 20;
        for code in [Key::Down, Key::Char('j')] {
            let before = offset(&overlay);
            assert_eq!(press(&mut overlay, code), Some(Msg::HelpScrolled));
            assert_eq!(offset(&overlay), before + 1);
        }
        for _ in 0..100 {
            press(&mut overlay, Key::Down);
        }
        assert_eq!(offset(&overlay), last);
        assert_eq!(press(&mut overlay, Key::Char('j')), None, "nothing moved");
        assert_eq!(offset(&overlay), last);
    }

    #[test]
    fn the_up_keys_scroll_one_row_and_clamp_at_the_top() {
        let mut overlay = with_interior(20);
        for _ in 0..5 {
            press(&mut overlay, Key::Down);
        }
        for code in [Key::Up, Key::Char('k')] {
            let before = offset(&overlay);
            assert_eq!(press(&mut overlay, code), Some(Msg::HelpScrolled));
            assert_eq!(offset(&overlay), before - 1);
        }
        assert_eq!(offset(&overlay), 3);
        for _ in 0..10 {
            press(&mut overlay, Key::Up);
        }
        assert_eq!(offset(&overlay), 0);
        assert_eq!(press(&mut overlay, Key::Char('k')), None);
    }

    fn wheel(overlay: &mut HelpOverlay, kind: MouseEventKind) -> Option<Msg> {
        let mouse = RoutedMouse {
            kind,
            column: 5,
            row: 4,
            area: Size::new(72, 22),
        };
        overlay.on(&Event::User(UserEvent::Mouse(mouse)))
    }

    #[test]
    fn the_wheel_scrolls_three_rows_per_step_and_clamps_at_both_ends() {
        let mut overlay = with_interior(20);
        let last = ROWS - 20;
        assert_eq!(
            wheel(&mut overlay, MouseEventKind::ScrollDown),
            Some(Msg::HelpScrolled)
        );
        assert_eq!(offset(&overlay), WHEEL_ROWS);
        assert_eq!(WHEEL_ROWS, 3);
        wheel(&mut overlay, MouseEventKind::ScrollDown);
        assert_eq!(offset(&overlay), 6);
        assert_eq!(
            wheel(&mut overlay, MouseEventKind::ScrollUp),
            Some(Msg::HelpScrolled)
        );
        assert_eq!(offset(&overlay), 3);
        for _ in 0..20 {
            wheel(&mut overlay, MouseEventKind::ScrollDown);
        }
        assert_eq!(offset(&overlay), last);
        assert_eq!(wheel(&mut overlay, MouseEventKind::ScrollDown), None);
        for _ in 0..20 {
            wheel(&mut overlay, MouseEventKind::ScrollUp);
        }
        assert_eq!(offset(&overlay), 0);
        assert_eq!(wheel(&mut overlay, MouseEventKind::ScrollUp), None);
    }

    #[test]
    fn the_wheel_shows_the_rows_it_scrolled_to() {
        let mut overlay = with_interior(20);
        let top = text_of(&mut overlay, 20);
        wheel(&mut overlay, MouseEventKind::ScrollDown);
        let scrolled = text_of(&mut overlay, 20);
        assert_ne!(top, scrolled);
        assert!(scrolled.iter().any(|row| row.contains("↑ more")));
    }

    #[test]
    fn clicks_and_the_raw_mouse_event_do_nothing() {
        let mut overlay = with_interior(20);
        for kind in [
            MouseEventKind::Down(tuirealm::event::MouseButton::Left),
            MouseEventKind::Down(tuirealm::event::MouseButton::Right),
            MouseEventKind::Up(tuirealm::event::MouseButton::Left),
            MouseEventKind::Moved,
            MouseEventKind::ScrollLeft,
        ] {
            assert_eq!(wheel(&mut overlay, kind), None, "{kind:?}");
        }
        let raw = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            modifiers: KeyModifiers::NONE,
            column: 5,
            row: 4,
        });
        assert_eq!(overlay.on(&raw), None);
        assert_eq!(offset(&overlay), 0);
    }

    #[test]
    fn the_page_keys_scroll_by_the_rows_of_the_interior_and_clamp() {
        let mut overlay = with_interior(20);
        assert_eq!(press(&mut overlay, Key::PageDown), Some(Msg::HelpScrolled));
        assert_eq!(offset(&overlay), 20);
        press(&mut overlay, Key::PageDown);
        assert_eq!(offset(&overlay), ROWS - 20, "clamped to the last page");
        press(&mut overlay, Key::PageUp);
        assert_eq!(offset(&overlay), ROWS - 40);
        press(&mut overlay, Key::PageUp);
        assert_eq!(offset(&overlay), 0);
        assert_eq!(press(&mut overlay, Key::PageUp), None);
    }

    #[test]
    fn a_remapped_scroll_key_replaces_the_default() {
        let mut raw = crate::config::RawKeys::new();
        raw.entry("overlay".to_owned())
            .or_default()
            .insert("down".to_owned(), vec!["d".to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut overlay = overlay_with(keymap, true);
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, 20)));
        assert_eq!(press(&mut overlay, Key::Char('d')), Some(Msg::HelpScrolled));
        assert_eq!(press(&mut overlay, Key::Char('j')), None);
        assert_eq!(offset(&overlay), 1);
    }

    #[test]
    fn nothing_scrolls_when_everything_fits_or_the_size_is_unknown() {
        let mut overlay = overlay();
        assert_eq!(press(&mut overlay, Key::Down), None, "no interior yet");
        assert_eq!(press(&mut overlay, Key::PageDown), None);
        let mut fits = with_interior(60);
        assert_eq!(press(&mut fits, Key::Down), None);
        assert_eq!(press(&mut fits, Key::PageDown), None);
        assert_eq!(offset(&fits), 0);
        let mut empty = with_interior(0);
        press(&mut empty, Key::PageDown);
        assert_eq!(offset(&empty), 1, "a zero page still moves one row");
    }

    #[test]
    fn a_taller_interior_pulls_the_offset_back_into_range() {
        let mut overlay = with_interior(20);
        press(&mut overlay, Key::PageDown);
        press(&mut overlay, Key::PageDown);
        assert_eq!(offset(&overlay), ROWS - 20);
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, 45)));
        assert_eq!(offset(&overlay), ROWS - 45);
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, 60)));
        assert_eq!(offset(&overlay), 0);
    }

    #[test]
    fn esc_and_the_help_key_close_it() {
        let mut overlay = overlay();
        assert_eq!(press(&mut overlay, Key::Esc), Some(Msg::CloseHelp));
        assert_eq!(press(&mut overlay, Key::Char('?')), Some(Msg::CloseHelp));
        let shifted = Event::Keyboard(KeyEvent::new(Key::Char('?'), KeyModifiers::SHIFT));
        assert_eq!(overlay.on(&shifted), Some(Msg::CloseHelp));
    }

    #[test]
    fn other_keys_do_nothing() {
        let mut overlay = with_interior(20);
        for code in [Key::Char('q'), Key::Char('t'), Key::Enter, Key::Char('x')] {
            assert_eq!(press(&mut overlay, code), None, "{code:?}");
        }
        let ctrl_o = Event::Keyboard(KeyEvent::new(Key::Char('o'), KeyModifiers::CONTROL));
        assert_eq!(overlay.on(&ctrl_o), None);
        assert_eq!(overlay.on(&Event::Tick), None);
        assert_eq!(offset(&overlay), 0);
    }

    #[test]
    fn a_remapped_help_key_closes_it() {
        let mut raw = crate::config::RawKeys::new();
        raw.entry("global".to_owned())
            .or_default()
            .insert("help".to_owned(), vec!["f1".to_owned()]);
        let (keymap, _) = Keymap::from_config(&raw);
        let mut overlay = overlay_with(keymap, true);
        assert_eq!(press(&mut overlay, Key::Function(1)), Some(Msg::CloseHelp));
        assert_eq!(press(&mut overlay, Key::Char('?')), None);
    }

    #[test]
    fn the_top_shows_a_down_indicator_only() {
        let mut overlay = with_interior(20);
        let rows = text_of(&mut overlay, 20);
        assert!(rows[1].ends_with("─╮") || rows[1].contains("╮"));
        assert!(!rows[1].contains("↑ more"), "{}", rows[1]);
        assert!(rows[22].contains(" ↓ more ─╯"), "{}", rows[22]);
    }

    #[test]
    fn the_middle_shows_both_indicators() {
        let mut overlay = with_interior(20);
        press(&mut overlay, Key::PageDown);
        let rows = text_of(&mut overlay, 20);
        assert!(rows[1].contains(" ↑ more ─╮"), "{}", rows[1]);
        assert!(rows[22].contains(" ↓ more ─╯"), "{}", rows[22]);
    }

    #[test]
    fn the_end_shows_an_up_indicator_and_the_shift_note() {
        let mut overlay = with_interior(20);
        press(&mut overlay, Key::PageDown);
        press(&mut overlay, Key::PageDown);
        let rows = text_of(&mut overlay, 20);
        assert!(rows[1].contains(" ↑ more ─╮"), "{}", rows[1]);
        assert!(!rows[22].contains("↓ more"), "{}", rows[22]);
        assert!(rows[21].contains(SHIFT_NOTE), "{}", rows[21]);
    }

    #[test]
    fn a_tall_enough_overlay_shows_no_indicator() {
        let rows_needed = u16::try_from(ROWS).unwrap();
        let mut overlay = with_interior(rows_needed);
        let rows = text_of(&mut overlay, rows_needed);
        assert!(rows.iter().all(|r| !r.contains("more")));
        assert!(rows[1].contains("╭─ Keybindings ─"));
        assert!(rows[ROWS + 2].contains("╰"));
    }

    #[test]
    fn the_indicators_follow_the_drawn_height_even_before_the_size_is_announced() {
        // The interior is unknown, but the area it is drawn in is not.
        let mut overlay = overlay();
        let rows = text_of(&mut overlay, 20);
        assert!(rows[22].contains(" ↓ more ─╯"), "{}", rows[22]);
    }

    #[test]
    fn the_indicators_are_muted_on_the_ring_border() {
        let theme = builtin::default_theme();
        let mut overlay = with_interior(20);
        press(&mut overlay, Key::PageDown);
        let buffer = draw(&mut overlay, 20);
        let top = &buffer[(10 + 72 - 4, 1)];
        assert_eq!(top.symbol(), "e", "the end of ` ↑ more `");
        assert_eq!(top.fg, theme.role(Role::MutedForeground));
        let dash = &buffer[(10 + 72 - 2, 1)];
        assert_eq!(dash.symbol(), "─");
        assert_eq!(dash.fg, theme.role(Role::Ring));
        let corner = &buffer[(10 + 72 - 1, 1)];
        assert_eq!(corner.symbol(), "╮");
        assert_eq!(corner.fg, theme.role(Role::Ring));
    }

    #[test]
    fn the_colours_come_from_the_theme() {
        let theme = builtin::default_theme();
        let mut overlay = with_interior(50);
        let buffer = draw(&mut overlay, 50);
        // Border and fill.
        assert_eq!(buffer[(10, 1)].fg, theme.role(Role::Ring));
        assert_eq!(buffer[(10, 1)].bg, theme.role(Role::Surface));
        assert_eq!(buffer[(30, 20)].bg, theme.role(Role::Surface));
        // The heading is bold foreground at column 3 of the overlay.
        let heading = &buffer[(10 + 3, 3)];
        assert_eq!(heading.symbol(), "N");
        assert_eq!(heading.fg, theme.role(Role::Foreground));
        assert!(heading.modifier.contains(ratatui::style::Modifier::BOLD));
        // Keys at column 5 in help_key, the description in help_text.
        let key = &buffer[(10 + 5, 5)];
        assert_eq!(key.symbol(), "U");
        assert_eq!(key.fg, theme.style(Token::HelpKey).fg.unwrap());
        let description = &buffer[(10 + 5 + 34, 5)];
        assert_eq!(description.symbol(), "P");
        assert_eq!(description.fg, theme.style(Token::HelpText).fg.unwrap());
        // Title in foreground.
        assert_eq!(buffer[(10 + 3, 1)].symbol(), "K");
        assert_eq!(buffer[(10 + 3, 1)].fg, theme.role(Role::Foreground));
    }

    #[test]
    fn a_transparent_theme_clears_but_does_not_fill() {
        let theme = builtin::default_theme().with_transparent_background(true);
        let keymap = Keymap::default();
        let content = HelpContent::new(&keymap, HelpContext::Browser, &[], true);
        let mut overlay = HelpOverlay::new(content, theme, keymap);
        let area = Rect::new(2, 1, 72, 12);
        let buffer = render(80, 14, |frame| {
            frame.render_widget(Paragraph::new("x".repeat(80 * 14)), frame.area());
            overlay.view(frame, area);
        });
        assert_eq!(buffer[(30, 6)].symbol(), " ", "the text under it is gone");
        assert_eq!(buffer[(30, 6)].bg, ratatui::style::Color::Reset);
        assert_eq!(buffer[(2, 1)].symbol(), "╭");
    }

    #[test]
    fn the_overlay_draws_within_a_small_area_and_a_degenerate_one() {
        let mut overlay = with_interior(3);
        for (w, h) in [(0, 0), (1, 1), (2, 2), (5, 3), (72, 5)] {
            let _ = render(80, 24, |frame| overlay.view(frame, Rect::new(1, 1, w, h)));
        }
    }

    #[test]
    fn the_theme_attribute_replaces_the_theme() {
        let mut overlay = overlay();
        let other = builtin::default_theme().with_transparent_background(true);
        overlay.attr(THEME_ATTR, theme_value(&other));
        let value = overlay.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&value), Some(other));
    }

    #[test]
    fn the_interior_attribute_is_kept() {
        let mut overlay = overlay();
        assert!(overlay.query(INTERIOR_ATTR).is_none());
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, 12)));
        let value = overlay.query(INTERIOR_ATTR).unwrap().into_attr();
        assert_eq!(payload_from_value::<Size>(&value), Some(Size::new(60, 12)));
    }

    #[test]
    fn the_overlay_snapshot_at_a_scrolled_position_is_styled() {
        let mut overlay = with_interior(12);
        press(&mut overlay, Key::PageDown);
        let area = Rect::new(0, 0, 72, 14);
        let buffer = render(72, 14, |frame| overlay.view(frame, area));
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }
}
