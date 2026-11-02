//! The details overlay (SPEC §5): the metadata of one section, in groups of
//! `label  value` rows, in a centred rounded overlay that scrolls when it is
//! taller than the screen.
//!
//! It shares its frame, scrolling and keys with the help overlay (see
//! [`OverlayFrame`]). The content is a snapshot, taken when the overlay
//! opens: nothing updates it while it is open.

mod content;
mod rows;
mod source;

#[cfg(test)]
mod snapshots;

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};

pub use content::{DetailsContent, DetailsGroup, DetailsRow};
pub use rows::section_details;
pub use source::{DetailsRequest, compose, section_source};

use super::overlay_frame::{FrameKey, INTERIOR_ATTR, OverlayFrame, interior_value};
use super::{THEME_ATTR, payload_from_value, payload_value, theme_from_value, theme_value};
use crate::app::{Msg, RoutedMouse, UserEvent};
use crate::keymap::{Action, Context, KeyResolver, Keymap, Resolved};
use crate::theme::Theme;

/// The attribute that tells the overlay the most columns it may be wide, the
/// screen's less its margin (a `u16` payload, see [`max_width_value`]).
pub const MAX_WIDTH_ATTR: Attribute = Attribute::Custom("details_max_width");
/// The attribute the overlay answers its wanted size with, borders included
/// (a [`Size`] payload): as wide as the widest row plus eight columns, but at
/// most [`MAX_WIDTH_ATTR`], and as high as the content wrapped at that width.
pub const DESIRED_SIZE_ATTR: Attribute = Attribute::Custom("details_desired_size");

/// The attribute value that sets the most columns the overlay may be wide.
pub fn max_width_value(width: u16) -> AttrValue {
    payload_value(width)
}

/// The details overlay. It takes the focus; `Esc` or the `details` key
/// closes it, the up and down keys scroll a row and the page keys a page.
///
/// `view` gets the overlay's rect (see `App`'s layout) as its area. Values
/// wider than the interior are wrapped at the width the overlay is drawn
/// with; the rows that scrolling counts are those of the announced interior
/// (see [`INTERIOR_ATTR`]).
#[derive(Debug, Clone)]
pub struct DetailsOverlay {
    content: DetailsContent,
    frame: OverlayFrame,
    /// Resolves the `details` key, which can be a two-key chord.
    resolver: KeyResolver,
    /// The most columns the overlay may be wide.
    max_width: u16,
}

impl DetailsOverlay {
    /// An overlay that shows `content`, resolving keys with `keymap`.
    pub fn new(content: DetailsContent, theme: Theme, keymap: Keymap) -> Self {
        Self {
            content,
            frame: OverlayFrame::new("Details", theme, keymap),
            resolver: KeyResolver::new(Context::Browser),
            max_width: u16::MAX,
        }
    }

    /// The size the overlay wants, borders included, when it may be at most
    /// `max_width` columns wide (see [`DetailsContent::desired_size`]).
    pub fn desired_size(&self, max_width: u16) -> Size {
        self.content.desired_size(max_width)
    }

    /// The rows of the content at the announced interior width; nothing
    /// scrolls until it is announced.
    fn total(&self) -> usize {
        self.frame
            .interior()
            .map_or(0, |size| self.content.row_count(size.width))
    }
}

impl Component for DetailsOverlay {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let theme = self.frame.theme();
        self.frame
            .draw(frame, area, |width| self.content.lines(theme, width));
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        if attr == THEME_ATTR {
            Some(QueryResult::Owned(theme_value(self.frame.theme())))
        } else if attr == INTERIOR_ATTR {
            self.frame
                .interior()
                .map(|size| QueryResult::Owned(interior_value(size)))
        } else if attr == DESIRED_SIZE_ATTR {
            let size = self.desired_size(self.max_width);
            Some(QueryResult::Owned(payload_value(size)))
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
            self.frame
                .set_interior(size, self.content.row_count(size.width));
        } else if attr == MAX_WIDTH_ATTR
            && let Some(width) = payload_from_value::<u16>(&value)
        {
            self.max_width = width;
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

impl DetailsOverlay {
    /// The wheel scrolls [`WHEEL_ROWS`](super::overlay_frame::WHEEL_ROWS)
    /// rows; clicks do nothing (the `App` closes the overlay for one outside
    /// it).
    fn on_mouse(&mut self, mouse: &RoutedMouse) -> Option<Msg> {
        let total = self.total();
        self.frame
            .on_mouse(mouse, total)
            .then_some(Msg::DetailsScrolled)
    }
}

impl AppComponent<Msg, UserEvent> for DetailsOverlay {
    /// A `details` chord that this key completes closes the overlay, whatever
    /// the key means in the overlay. Otherwise the overlay keys come first
    /// (`Esc` closes, the up, down and page keys scroll), so a single
    /// `details` key that is also an overlay key does what the overlay says;
    /// then the `details` key closes it too. Every other key does nothing:
    /// the keys of the transcript and the global keys are not for the
    /// overlay.
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let key = match ev {
            Event::Keyboard(key) => key,
            // Only what the `App` routed here, in the overlay's coordinates.
            Event::User(UserEvent::Mouse(mouse)) => return self.on_mouse(mouse),
            _ => return None,
        };
        // The `details` key has a resolver of its own, because it can be a
        // chord. It sees every key once, so an overlay key can start or end a
        // chord, and a key that does not complete one drops the prefix.
        let keymap = self.frame.keymap();
        let completes_chord = self.resolver.completes(keymap, key, Action::Details);
        let resolved = self.resolver.feed(keymap, key);
        if completes_chord {
            return Some(Msg::CloseDetails);
        }
        let total = self.total();
        match self.frame.on_key(key, total) {
            FrameKey::Close => Some(Msg::CloseDetails),
            FrameKey::Scrolled => Some(Msg::DetailsScrolled),
            FrameKey::Unmoved => None,
            FrameKey::Unbound => {
                (resolved == Resolved::Action(Action::Details)).then_some(Msg::CloseDetails)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_text, render};
    use crate::theme::{Role, builtin};
    use tuirealm::event::{Key, KeyEvent, KeyModifiers, MouseEventKind};

    fn content() -> DetailsContent {
        let rows = (1..=30)
            .map(|n| DetailsRow::new(format!("row {n}"), format!("value {n}")))
            .collect();
        DetailsContent::new(vec![DetailsGroup::new("Turn", rows)])
    }

    fn overlay_with(keymap: Keymap, rows: u16) -> DetailsOverlay {
        let mut overlay = DetailsOverlay::new(content(), builtin::default_theme(), keymap);
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, rows)));
        overlay
    }

    fn overlay(rows: u16) -> DetailsOverlay {
        overlay_with(Keymap::default(), rows)
    }

    fn press(overlay: &mut DetailsOverlay, code: Key) -> Option<Msg> {
        overlay.on(&Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE)))
    }

    fn offset(overlay: &DetailsOverlay) -> usize {
        match overlay.state() {
            State::Single(StateValue::Usize(offset)) => offset,
            other => panic!("{other:?}"),
        }
    }

    fn wheel(overlay: &mut DetailsOverlay, kind: MouseEventKind) -> Option<Msg> {
        let mouse = RoutedMouse {
            kind,
            column: 5,
            row: 4,
            area: Size::new(60, 12),
        };
        overlay.on(&Event::User(UserEvent::Mouse(mouse)))
    }

    /// 30 rows, a blank row, the heading, a blank row and a blank row.
    const ROWS: usize = 34;

    #[test]
    fn esc_and_the_details_key_close_it() {
        let mut overlay = overlay(10);
        assert_eq!(press(&mut overlay, Key::Esc), Some(Msg::CloseDetails));
        assert_eq!(press(&mut overlay, Key::Char('i')), Some(Msg::CloseDetails));
    }

    #[test]
    fn a_remapped_details_key_closes_it_and_the_default_does_not() {
        let mut raw = crate::config::RawKeys::new();
        raw.entry("browser".to_owned())
            .or_default()
            .insert("details".to_owned(), vec!["x".to_owned()]);
        let (keymap, _) = Keymap::from_config(&raw);
        let mut overlay = overlay_with(keymap, 10);
        assert_eq!(press(&mut overlay, Key::Char('x')), Some(Msg::CloseDetails));
        assert_eq!(press(&mut overlay, Key::Char('i')), None);
    }

    fn chord_overlay() -> DetailsOverlay {
        let mut raw = crate::config::RawKeys::new();
        raw.entry("browser".to_owned())
            .or_default()
            .insert("details".to_owned(), vec!["x i".to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        overlay_with(keymap, 10)
    }

    #[test]
    fn a_details_key_bound_to_a_chord_closes_it() {
        let mut overlay = chord_overlay();
        assert_eq!(press(&mut overlay, Key::Char('x')), None);
        assert_eq!(press(&mut overlay, Key::Char('i')), Some(Msg::CloseDetails));
    }

    #[test]
    fn a_lone_prefix_or_a_second_key_alone_does_not_close_it() {
        let mut overlay = chord_overlay();
        assert_eq!(press(&mut overlay, Key::Char('i')), None);
        assert_eq!(press(&mut overlay, Key::Char('x')), None);
        assert_eq!(press(&mut overlay, Key::Char('y')), None);
        // The prefix was dropped by the key after it.
        assert_eq!(press(&mut overlay, Key::Char('i')), None);
    }

    #[test]
    fn an_overlay_key_drops_a_pending_prefix() {
        let mut overlay = chord_overlay();
        assert_eq!(press(&mut overlay, Key::Char('x')), None);
        assert_eq!(press(&mut overlay, Key::Down), Some(Msg::DetailsScrolled));
        assert_eq!(press(&mut overlay, Key::Char('i')), None);
        assert_eq!(press(&mut overlay, Key::Char('x')), None);
        assert_eq!(press(&mut overlay, Key::Esc), Some(Msg::CloseDetails));
        assert_eq!(press(&mut overlay, Key::Char('i')), None);
    }

    #[test]
    fn it_answers_the_size_it_wants_for_the_width_it_may_have() {
        let mut overlay = overlay(10);
        let wanted = |overlay: &DetailsOverlay| {
            let value = overlay.query(DESIRED_SIZE_ATTR).unwrap().into_attr();
            payload_from_value::<Size>(&value).unwrap()
        };
        assert_eq!(wanted(&overlay), overlay.desired_size(u16::MAX));
        overlay.attr(MAX_WIDTH_ATTR, max_width_value(30));
        assert_eq!(wanted(&overlay), overlay.desired_size(30));
        assert_eq!(wanted(&overlay).width, 30);
    }

    /// An overlay whose browser keys are `bindings` (action, key).
    fn overlay_bound(bindings: &[(&str, &str)]) -> DetailsOverlay {
        let mut raw = crate::config::RawKeys::new();
        let browser = raw.entry("browser".to_owned()).or_default();
        for (action, key) in bindings {
            browser.insert((*action).to_owned(), vec![(*key).to_owned()]);
        }
        let (keymap, _) = Keymap::from_config(&raw);
        overlay_with(keymap, 10)
    }

    #[test]
    fn a_chord_that_ends_in_an_overlay_key_closes_it() {
        let mut overlay = overlay_bound(&[("details", "x j")]);
        assert_eq!(press(&mut overlay, Key::Char('x')), None);
        assert_eq!(press(&mut overlay, Key::Char('j')), Some(Msg::CloseDetails));
        assert_eq!(offset(&overlay), 0);
    }

    #[test]
    fn the_overlay_key_of_such_a_chord_scrolls_without_the_prefix() {
        let mut overlay = overlay_bound(&[("details", "x j")]);
        assert_eq!(
            press(&mut overlay, Key::Char('j')),
            Some(Msg::DetailsScrolled)
        );
        assert_eq!(offset(&overlay), 1);
    }

    #[test]
    fn a_chord_that_starts_with_an_overlay_key_scrolls_and_then_closes_it() {
        let mut overlay = overlay_bound(&[("details", "j i")]);
        assert_eq!(press(&mut overlay, Key::Char('i')), None);
        assert_eq!(
            press(&mut overlay, Key::Char('j')),
            Some(Msg::DetailsScrolled)
        );
        assert_eq!(offset(&overlay), 1);
        assert_eq!(press(&mut overlay, Key::Char('i')), Some(Msg::CloseDetails));
    }

    #[test]
    fn the_prefix_of_another_chord_does_not_make_a_single_overlay_key_close_it() {
        let mut overlay = overlay_bound(&[("details", "j"), ("top", "g g")]);
        assert_eq!(press(&mut overlay, Key::Char('g')), None);
        assert_eq!(
            press(&mut overlay, Key::Char('j')),
            Some(Msg::DetailsScrolled)
        );
        assert_eq!(offset(&overlay), 1);
    }

    #[test]
    fn the_overlay_keys_come_first() {
        // `j` scrolls; it is not shadowed when the details key is `j`.
        let mut raw = crate::config::RawKeys::new();
        raw.entry("browser".to_owned())
            .or_default()
            .insert("details".to_owned(), vec!["j".to_owned()]);
        let (keymap, _) = Keymap::from_config(&raw);
        let mut overlay = overlay_with(keymap, 10);
        assert_eq!(
            press(&mut overlay, Key::Char('j')),
            Some(Msg::DetailsScrolled)
        );
        assert_eq!(offset(&overlay), 1);
    }

    #[test]
    fn other_keys_do_nothing() {
        let mut overlay = overlay(10);
        // The help key does not close it, and the keys of the transcript
        // and the global keys are not for it.
        for code in [
            Key::Char('?'),
            Key::Char('q'),
            Key::Char('t'),
            Key::Char('y'),
            Key::Char('w'),
            Key::Char('e'),
            Key::Enter,
            Key::Char('x'),
        ] {
            assert_eq!(press(&mut overlay, code), None, "{code:?}");
        }
        let ctrl_o = Event::Keyboard(KeyEvent::new(Key::Char('o'), KeyModifiers::CONTROL));
        assert_eq!(overlay.on(&ctrl_o), None);
        assert_eq!(overlay.on(&Event::Tick), None);
        assert_eq!(offset(&overlay), 0);
    }

    #[test]
    fn the_scroll_keys_move_by_a_row_or_a_page_and_clamp() {
        let mut overlay = overlay(10);
        for code in [Key::Down, Key::Char('j')] {
            let before = offset(&overlay);
            assert_eq!(press(&mut overlay, code), Some(Msg::DetailsScrolled));
            assert_eq!(offset(&overlay), before + 1);
        }
        assert_eq!(
            press(&mut overlay, Key::PageDown),
            Some(Msg::DetailsScrolled)
        );
        assert_eq!(offset(&overlay), 12);
        for _ in 0..5 {
            press(&mut overlay, Key::PageDown);
        }
        assert_eq!(offset(&overlay), ROWS - 10, "clamped to the last page");
        assert_eq!(press(&mut overlay, Key::Down), None, "nothing moved");
        assert_eq!(press(&mut overlay, Key::PageUp), Some(Msg::DetailsScrolled));
        assert_eq!(offset(&overlay), ROWS - 20);
        for _ in 0..5 {
            press(&mut overlay, Key::Up);
        }
        assert_eq!(offset(&overlay), ROWS - 25);
        press(&mut overlay, Key::PageUp);
        press(&mut overlay, Key::PageUp);
        assert_eq!(offset(&overlay), 0);
        assert_eq!(press(&mut overlay, Key::Char('k')), None);
    }

    #[test]
    fn the_wheel_scrolls_three_rows_per_step_and_clamps() {
        let mut overlay = overlay(10);
        assert_eq!(
            wheel(&mut overlay, MouseEventKind::ScrollDown),
            Some(Msg::DetailsScrolled)
        );
        assert_eq!(offset(&overlay), 3);
        for _ in 0..20 {
            wheel(&mut overlay, MouseEventKind::ScrollDown);
        }
        assert_eq!(offset(&overlay), ROWS - 10);
        assert_eq!(wheel(&mut overlay, MouseEventKind::ScrollDown), None);
        assert_eq!(
            wheel(&mut overlay, MouseEventKind::ScrollUp),
            Some(Msg::DetailsScrolled)
        );
        assert_eq!(offset(&overlay), ROWS - 13);
    }

    #[test]
    fn clicks_and_the_raw_mouse_event_do_nothing() {
        let mut overlay = overlay(10);
        for kind in [
            MouseEventKind::Down(tuirealm::event::MouseButton::Left),
            MouseEventKind::Up(tuirealm::event::MouseButton::Left),
            MouseEventKind::Moved,
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
    fn nothing_scrolls_before_the_interior_is_announced_or_when_it_all_fits() {
        let mut unannounced =
            DetailsOverlay::new(content(), builtin::default_theme(), Keymap::default());
        assert_eq!(press(&mut unannounced, Key::Down), None);
        assert_eq!(press(&mut unannounced, Key::PageDown), None);
        let mut fits = overlay(40);
        assert_eq!(press(&mut fits, Key::Down), None);
        assert_eq!(wheel(&mut fits, MouseEventKind::ScrollDown), None);
        assert_eq!(offset(&fits), 0);
    }

    #[test]
    fn scrolling_follows_the_rows_at_the_announced_width() {
        // A value that wraps at a narrow interior takes more rows.
        let long = DetailsContent::new(vec![DetailsGroup::new(
            "Environment",
            vec![DetailsRow::new("cwd", "x".repeat(100))],
        )]);
        let mut overlay = DetailsOverlay::new(long, builtin::default_theme(), Keymap::default());
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(30, 4)));
        // Interior 30: 30 - 6 - 5 = 19 columns, 6 rows for the value.
        for _ in 0..20 {
            press(&mut overlay, Key::Down);
        }
        assert_eq!(offset(&overlay), 1 + 2 + 6 + 1 - 4);
        // A wide interior needs no wrap, so everything fits in 5 rows.
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(120, 5)));
        assert_eq!(offset(&overlay), 0);
    }

    #[test]
    fn a_taller_interior_pulls_the_offset_back_into_range() {
        let mut overlay = overlay(10);
        for _ in 0..5 {
            press(&mut overlay, Key::PageDown);
        }
        assert_eq!(offset(&overlay), ROWS - 10);
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, 30)));
        assert_eq!(offset(&overlay), ROWS - 30);
        overlay.attr(INTERIOR_ATTR, interior_value(Size::new(60, 40)));
        assert_eq!(offset(&overlay), 0);
    }

    fn drawn(overlay: &mut DetailsOverlay, rows: u16) -> Vec<String> {
        let area = Rect::new(2, 1, 60, rows + 2);
        let buffer = render(70, rows + 4, |frame| overlay.view(frame, area));
        buffer_to_text(&buffer).lines().map(str::to_owned).collect()
    }

    #[test]
    fn the_frame_is_titled_details_and_shows_where_more_is() {
        let mut overlay = overlay(10);
        let top = drawn(&mut overlay, 10);
        assert!(top[1].contains("╭─ Details ─"), "{}", top[1]);
        assert!(!top[1].contains("↑ more"), "{}", top[1]);
        assert!(top[12].contains(" ↓ more ─╯"), "{}", top[12]);
        press(&mut overlay, Key::PageDown);
        let middle = drawn(&mut overlay, 10);
        assert!(middle[1].contains(" ↑ more ─╮"), "{}", middle[1]);
        assert!(middle[12].contains(" ↓ more ─╯"), "{}", middle[12]);
        for _ in 0..5 {
            press(&mut overlay, Key::PageDown);
        }
        let end = drawn(&mut overlay, 10);
        assert!(end[1].contains(" ↑ more ─╮"), "{}", end[1]);
        assert!(!end[12].contains("↓ more"), "{}", end[12]);
    }

    #[test]
    fn a_tall_enough_overlay_shows_no_indicator() {
        let mut overlay = overlay(40);
        let rows = drawn(&mut overlay, 40);
        assert!(rows.iter().all(|r| !r.contains("more")));
    }

    #[test]
    fn the_rows_are_drawn_from_the_offset() {
        let mut overlay = overlay(10);
        assert!(drawn(&mut overlay, 10)[5].contains("row 1 "));
        press(&mut overlay, Key::PageDown);
        assert!(drawn(&mut overlay, 10)[2].contains("row 8 "));
    }

    #[test]
    fn the_colours_come_from_the_theme() {
        let theme = builtin::default_theme();
        let mut overlay = overlay(40);
        let area = Rect::new(2, 1, 60, 42);
        let buffer = render(70, 44, |frame| overlay.view(frame, area));
        assert_eq!(buffer[(2, 1)].fg, theme.role(Role::Ring));
        assert_eq!(buffer[(2, 1)].bg, theme.role(Role::Surface));
        assert_eq!(buffer[(30, 20)].bg, theme.role(Role::Surface));
        // Heading at overlay column 3, rows at column 5.
        let heading = &buffer[(2 + 3, 3)];
        assert_eq!(heading.symbol(), "T");
        assert_eq!(heading.fg, theme.role(Role::Foreground));
        assert!(heading.modifier.contains(ratatui::style::Modifier::BOLD));
        let label = &buffer[(2 + 5, 5)];
        assert_eq!(label.symbol(), "r");
        assert_eq!(
            label.fg,
            theme.style(crate::theme::Token::Label).fg.unwrap()
        );
        // `row 10` is the widest label (6): the value is at column 5 + 8.
        let value = &buffer[(2 + 5 + 8, 5)];
        assert_eq!(value.symbol(), "v");
        assert_eq!(value.fg, theme.role(Role::Foreground));
    }

    #[test]
    fn a_transparent_theme_clears_but_does_not_fill() {
        let theme = builtin::default_theme().with_transparent_background(true);
        let mut overlay = DetailsOverlay::new(content(), theme, Keymap::default());
        let area = Rect::new(2, 1, 40, 12);
        let buffer = render(60, 14, |frame| {
            frame.render_widget(
                ratatui::widgets::Paragraph::new("x".repeat(60 * 14)),
                frame.area(),
            );
            overlay.view(frame, area);
        });
        assert_eq!(buffer[(20, 6)].symbol(), " ", "the text under it is gone");
        assert_eq!(buffer[(20, 6)].bg, ratatui::style::Color::Reset);
        assert_eq!(buffer[(2, 1)].symbol(), "╭");
    }

    #[test]
    fn it_draws_within_a_small_area_and_a_degenerate_one() {
        let mut overlay = overlay(3);
        for (w, h) in [(0, 0), (1, 1), (2, 2), (5, 3), (7, 5), (72, 5)] {
            let _ = render(80, 24, |frame| overlay.view(frame, Rect::new(1, 1, w, h)));
        }
    }

    #[test]
    fn the_theme_and_interior_attributes_are_kept() {
        let mut overlay = overlay(10);
        let other = builtin::default_theme().with_transparent_background(true);
        overlay.attr(THEME_ATTR, theme_value(&other));
        let value = overlay.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&value), Some(other));
        let value = overlay.query(INTERIOR_ATTR).unwrap().into_attr();
        assert_eq!(payload_from_value::<Size>(&value), Some(Size::new(60, 10)));
        let fresh = DetailsOverlay::new(content(), builtin::default_theme(), Keymap::default());
        assert!(fresh.query(INTERIOR_ATTR).is_none());
    }
}
