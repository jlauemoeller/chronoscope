//! The theme selector overlay (SPEC §8.4): a small centred list of the
//! registry's themes. Moving the cursor previews the highlighted theme,
//! `Enter` keeps it and `Esc` gives the old one back; the `App` does the
//! applying and saving, this component says what the user asked for.

mod rows;

use ratatui::Frame;
use ratatui::layout::{Rect, Size};
use ratatui::style::Style;
use ratatui::widgets::{Block, Clear};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};
use unicode_width::UnicodeWidthStr;

pub use rows::{ThemeItem, ThemeRows};

use super::list_pane::{ListCursor, VISIBLE_ROWS_ATTR, titled_block};
use super::{HelpLine, Hint, THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::keymap::{Action, Context, Keymap};
use crate::theme::{Role, Theme, ThemeRegistry};

/// The overlay is at least this wide.
pub const MIN_WIDTH: u16 = 41;
/// The columns the overlay is wider than its longest label: the label starts
/// in column 5, and leaves as many columns free after it.
const WIDTH_CHROME: usize = 10;
/// The rows of the overlay that are not theme rows: the borders, a blank row
/// above the list and one below it, and the help row.
const CHROME_ROWS: u16 = 5;
/// The columns the help row is indented from the border.
const HELP_INSET: u16 = 1;

/// The items for `registry`'s themes, in the registry's order.
pub fn theme_items(registry: &ThemeRegistry) -> Vec<ThemeItem> {
    registry
        .entries()
        .iter()
        .map(|entry| {
            if entry.is_selectable() {
                ThemeItem::valid(entry.id(), entry.label())
            } else {
                ThemeItem::invalid(entry.id())
            }
        })
        .collect()
}

/// The theme selector. It takes the focus.
///
/// Up and down move the cursor over the themes that can be selected (no
/// wrap), each move saying `Msg::PreviewTheme`. `select` says
/// `Msg::SelectTheme` for the theme under the cursor and `close` says
/// `Msg::CancelTheme`; the keys come from the keymap's `overlay` context.
///
/// `view` gets the overlay's rect (see `App`'s layout) as its area. The
/// overlay is cleared and filled with `surface` (only cleared with a
/// transparent theme), and bordered in `ring`. When there are more themes than
/// rows the list scrolls, by the least that keeps the cursor visible; the
/// number of rows is announced with [`VISIBLE_ROWS_ATTR`].
#[derive(Debug, Clone)]
pub struct ThemeSelector {
    items: Vec<ThemeItem>,
    cursor: ListCursor,
    /// The rows the list showed last time it was announced; 0 while unknown.
    rows: usize,
    theme: Theme,
    keymap: Keymap,
}

impl ThemeSelector {
    /// A selector that lists `items`, with the cursor on the one whose id is
    /// `active` (or the first that can be selected, if none is).
    pub fn new(items: Vec<ThemeItem>, active: &str, theme: Theme, keymap: Keymap) -> Self {
        let start = items
            .iter()
            .position(|item| item.selectable && item.id == active)
            .or_else(|| items.iter().position(|item| item.selectable));
        let mut cursor = ListCursor::default();
        cursor.set(items.len(), start);
        Self {
            items,
            cursor,
            rows: 0,
            theme,
            keymap,
        }
    }

    /// The size the overlay wants, borders included: as wide as its longest
    /// label plus ten columns (at least 41), and a row per theme plus five.
    pub fn desired_size(&self) -> Size {
        let longest = self
            .items
            .iter()
            .map(|i| i.label.width())
            .max()
            .unwrap_or(0);
        let width = u16::try_from(longest + WIDTH_CHROME).unwrap_or(u16::MAX);
        let rows = u16::try_from(self.items.len()).unwrap_or(u16::MAX);
        Size::new(width.max(MIN_WIDTH), rows.saturating_add(CHROME_ROWS))
    }

    /// How many theme rows an overlay of `overlay_height` rows shows.
    pub fn rows_in(overlay_height: u16) -> usize {
        usize::from(overlay_height.saturating_sub(CHROME_ROWS))
    }

    /// The id of the theme under the cursor.
    pub fn selected_id(&self) -> Option<&str> {
        let item = self.items.get(self.cursor.selected()?)?;
        Some(&item.id)
    }

    /// The row the cursor would move to from `from` in the given direction,
    /// skipping the rows that cannot be selected.
    fn neighbour(&self, from: usize, down: bool) -> Option<usize> {
        if down {
            (from + 1..self.items.len()).find(|&i| self.items[i].selectable)
        } else {
            (0..from).rev().find(|&i| self.items[i].selectable)
        }
    }

    /// Moves the cursor one theme up or down. Returns whether it moved; at
    /// the first or last theme it does not.
    fn step(&mut self, down: bool) -> bool {
        let Some(target) = self
            .cursor
            .selected()
            .and_then(|from| self.neighbour(from, down))
        else {
            return false;
        };
        self.cursor.select(Some(target));
        true
    }

    fn block(&self) -> Block<'static> {
        let mut block = titled_block(&self.theme, "Themes", Role::Ring);
        if !self.theme.transparent_background() {
            block = block.style(Style::new().bg(self.theme.role(Role::Surface)));
        }
        block
    }

    /// The hints of the help row, from the keymap so remapped keys show up.
    fn hints(&self) -> Vec<Hint> {
        [(Action::Select, "to select"), (Action::Close, "to cancel")]
            .into_iter()
            .filter_map(|(action, text)| {
                let key = self.keymap.label(Context::Overlay, action)?;
                Some(Hint::new(key, text))
            })
            .collect()
    }

    /// The list's area and the help row's, inside the border: a blank row,
    /// the list, a blank row and the help row.
    fn parts(inner: Rect) -> (Rect, Rect) {
        let list = Rect::new(
            inner.x,
            inner.y + 1.min(inner.height),
            inner.width,
            inner.height.saturating_sub(CHROME_ROWS - 2),
        );
        let help = Rect::new(inner.x, inner.bottom().saturating_sub(1), inner.width, 1);
        (list, if inner.height >= 3 { help } else { Rect::ZERO })
    }
}

impl Component for ThemeSelector {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        frame.render_widget(Clear, area);
        let block = self.block();
        let inner = block.inner(area);
        frame.render_widget(block, area);
        let (list, help) = Self::parts(inner);
        let rows = ThemeRows {
            items: &self.items,
            selected: self.cursor.selected(),
            offset: self.cursor.offset_for(usize::from(list.height)),
            theme: &self.theme,
        };
        frame.render_widget(rows, list);
        if !help.is_empty() {
            let mut line =
                HelpLine::new(self.hints(), Vec::new(), self.theme.clone(), Role::Surface)
                    .inset(HELP_INSET);
            line.view(frame, help);
        }
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            VISIBLE_ROWS_ATTR => Some(QueryResult::Owned(AttrValue::Length(self.rows))),
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
            (VISIBLE_ROWS_ATTR, AttrValue::Length(rows)) => {
                self.rows = rows;
                self.cursor.set_visible(rows);
            }
            _ => {}
        }
    }

    /// The id of the theme under the cursor.
    fn state(&self) -> State {
        self.selected_id().map_or(State::None, |id| {
            State::Single(StateValue::String(id.to_owned()))
        })
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for ThemeSelector {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        let Event::Keyboard(key) = ev else {
            return None;
        };
        match self.keymap.resolve(Context::Overlay, key)? {
            Action::Close => Some(Msg::CancelTheme),
            Action::Select => self.selected_id().map(|id| Msg::SelectTheme(id.to_owned())),
            Action::Up => self.moved(false),
            Action::Down => self.moved(true),
            _ => None,
        }
    }
}

impl ThemeSelector {
    /// The preview message for a move of the cursor, if it moved.
    fn moved(&mut self, down: bool) -> Option<Msg> {
        if !self.step(down) {
            return None;
        }
        self.selected_id()
            .map(|id| Msg::PreviewTheme(id.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::{Token, builtin};
    use ratatui::style::{Color, Modifier};
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};

    /// A complete user theme document named `name`.
    fn theme_toml(name: &str) -> String {
        let mut out = format!("name = \"{name}\"\n");
        for (i, role) in Role::ALL.iter().enumerate() {
            let v = u8::try_from(i).unwrap() + 1;
            out.push_str(&format!("{} = \"#{v:02x}{v:02x}{v:02x}\"\n", role.key()));
        }
        out
    }

    /// The built-ins plus these user themes (`(id, valid)`).
    fn registry_with(user: &[(&str, bool)]) -> ThemeRegistry {
        let dir = tempfile::tempdir().unwrap();
        for (id, valid) in user {
            let text = if *valid {
                theme_toml(&format!("User {id}"))
            } else {
                "name = \"broken\"\nforeground = \"not a colour\"\n".to_owned()
            };
            std::fs::write(dir.path().join(format!("{id}.toml")), text).unwrap();
        }
        ThemeRegistry::load(dir.path())
    }

    fn selector_for(registry: &ThemeRegistry, active: &str) -> ThemeSelector {
        ThemeSelector::new(
            theme_items(registry),
            active,
            builtin::default_theme(),
            Keymap::default(),
        )
    }

    fn selector() -> ThemeSelector {
        selector_for(&ThemeRegistry::builtin(), "terminal")
    }

    /// A selector of `n` themes named `Theme 0` ... with the cursor on the
    /// first.
    fn numbered(n: usize) -> ThemeSelector {
        let items = (0..n)
            .map(|i| ThemeItem::valid(format!("t{i}"), format!("Theme {i}")))
            .collect();
        ThemeSelector::new(items, "t0", builtin::default_theme(), Keymap::default())
    }

    fn press(selector: &mut ThemeSelector, code: Key) -> Option<Msg> {
        selector.on(&Event::Keyboard(KeyEvent::new(code, KeyModifiers::NONE)))
    }

    fn selected(selector: &ThemeSelector) -> Option<String> {
        match selector.state() {
            State::Single(StateValue::String(id)) => Some(id),
            State::None => None,
            other => panic!("{other:?}"),
        }
    }

    fn labels(selector: &ThemeSelector) -> Vec<&str> {
        selector.items.iter().map(|i| i.label.as_str()).collect()
    }

    /// The selector drawn the way the app places it on a 80x30 screen.
    fn draw(selector: &mut ThemeSelector) -> ratatui::buffer::Buffer {
        let size = selector.desired_size();
        let area = Rect::new(10, 2, size.width, size.height);
        selector.attr(
            VISIBLE_ROWS_ATTR,
            AttrValue::Length(ThemeSelector::rows_in(size.height)),
        );
        render(80, size.height + 4, |frame| selector.view(frame, area))
    }

    fn rows_of(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
        buffer_to_text(buffer).lines().map(str::to_owned).collect()
    }

    // -- the list

    #[test]
    fn the_builtins_come_in_the_order_of_the_spec_with_their_display_names() {
        let s = selector();
        assert_eq!(
            labels(&s),
            [
                "Default",
                "Terminal",
                "Catppuccin",
                "Gruvbox",
                "Nord",
                "Tokyo Night",
                "Solarized"
            ]
        );
        let ids: Vec<&str> = s.items.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "default",
                "terminal",
                "catppuccin",
                "gruvbox",
                "nord",
                "tokyo-night",
                "solarized"
            ]
        );
    }

    #[test]
    fn user_themes_follow_alphabetically_and_an_invalid_one_reads_invalid() {
        let registry = registry_with(&[("zeta", true), ("my-theme", false), ("alpha", true)]);
        let s = selector_for(&registry, "default");
        assert_eq!(
            labels(&s)[7..],
            ["User alpha", "my-theme (invalid)", "User zeta"]
        );
        let invalid = &s.items[8];
        assert_eq!(invalid.id, "my-theme");
        assert!(!invalid.selectable);
        assert!(s.items[7].selectable && s.items[9].selectable);
    }

    #[test]
    fn the_cursor_starts_on_the_active_theme() {
        let registry = registry_with(&[("alpha", true)]);
        for id in ["default", "terminal", "nord", "solarized", "alpha"] {
            assert_eq!(selected(&selector_for(&registry, id)).as_deref(), Some(id));
        }
    }

    #[test]
    fn an_active_theme_that_cannot_be_selected_starts_on_the_first_that_can() {
        let registry = registry_with(&[("broken", false)]);
        assert_eq!(
            selected(&selector_for(&registry, "broken")).as_deref(),
            Some("default")
        );
        assert_eq!(
            selected(&selector_for(&registry, "nope")).as_deref(),
            Some("default")
        );
        let none = ThemeSelector::new(
            vec![ThemeItem::invalid("a")],
            "a",
            builtin::default_theme(),
            Keymap::default(),
        );
        assert_eq!(selected(&none), None);
    }

    // -- keys

    #[test]
    fn the_down_keys_move_one_theme_and_say_to_preview_it() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "default");
        assert_eq!(
            press(&mut s, Key::Down),
            Some(Msg::PreviewTheme("terminal".into()))
        );
        assert_eq!(
            press(&mut s, Key::Char('j')),
            Some(Msg::PreviewTheme("catppuccin".into()))
        );
        assert_eq!(selected(&s).as_deref(), Some("catppuccin"));
    }

    #[test]
    fn the_up_keys_move_one_theme_and_say_to_preview_it() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "nord");
        assert_eq!(
            press(&mut s, Key::Up),
            Some(Msg::PreviewTheme("gruvbox".into()))
        );
        assert_eq!(
            press(&mut s, Key::Char('k')),
            Some(Msg::PreviewTheme("catppuccin".into()))
        );
    }

    #[test]
    fn the_cursor_does_not_wrap_and_says_nothing_at_the_ends() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "default");
        assert_eq!(press(&mut s, Key::Up), None);
        assert_eq!(selected(&s).as_deref(), Some("default"));
        let mut s = selector_for(&ThemeRegistry::builtin(), "solarized");
        assert_eq!(press(&mut s, Key::Down), None);
        assert_eq!(press(&mut s, Key::Char('j')), None);
        assert_eq!(selected(&s).as_deref(), Some("solarized"));
    }

    #[test]
    fn the_cursor_skips_invalid_themes_in_both_directions() {
        let registry = registry_with(&[("a-bad", false), ("b-ok", true), ("c-bad", false)]);
        let mut s = selector_for(&registry, "solarized");
        assert_eq!(
            press(&mut s, Key::Down),
            Some(Msg::PreviewTheme("b-ok".into()))
        );
        assert_eq!(press(&mut s, Key::Down), None, "c-bad is last and invalid");
        assert_eq!(selected(&s).as_deref(), Some("b-ok"));
        assert_eq!(
            press(&mut s, Key::Up),
            Some(Msg::PreviewTheme("solarized".into()))
        );
    }

    #[test]
    fn enter_selects_the_theme_under_the_cursor() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "gruvbox");
        assert_eq!(
            press(&mut s, Key::Enter),
            Some(Msg::SelectTheme("gruvbox".into()))
        );
        press(&mut s, Key::Down);
        assert_eq!(
            press(&mut s, Key::Enter),
            Some(Msg::SelectTheme("nord".into()))
        );
    }

    #[test]
    fn enter_with_nothing_selectable_says_nothing() {
        let mut s = ThemeSelector::new(
            vec![ThemeItem::invalid("a")],
            "a",
            builtin::default_theme(),
            Keymap::default(),
        );
        assert_eq!(press(&mut s, Key::Enter), None);
        assert_eq!(press(&mut s, Key::Down), None);
    }

    #[test]
    fn esc_cancels() {
        let mut s = selector();
        assert_eq!(press(&mut s, Key::Esc), Some(Msg::CancelTheme));
    }

    #[test]
    fn other_keys_and_events_do_nothing() {
        let mut s = selector();
        for code in [
            Key::Char('q'),
            Key::Char('t'),
            Key::Char('?'),
            Key::Char('x'),
            Key::Tab,
            Key::Left,
        ] {
            assert_eq!(press(&mut s, code), None, "{code:?}");
        }
        let ctrl_o = Event::Keyboard(KeyEvent::new(Key::Char('o'), KeyModifiers::CONTROL));
        assert_eq!(s.on(&ctrl_o), None);
        assert_eq!(s.on(&Event::Tick), None);
        assert_eq!(selected(&s).as_deref(), Some("terminal"));
    }

    #[test]
    fn remapped_overlay_keys_replace_the_defaults() {
        let mut raw = crate::config::RawKeys::new();
        let overlay = raw.entry("overlay".to_owned()).or_default();
        overlay.insert("down".to_owned(), vec!["d".to_owned()]);
        overlay.insert("select".to_owned(), vec!["space".to_owned()]);
        overlay.insert("close".to_owned(), vec!["x".to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut s = ThemeSelector::new(
            theme_items(&ThemeRegistry::builtin()),
            "default",
            builtin::default_theme(),
            keymap,
        );
        assert_eq!(
            press(&mut s, Key::Char('d')),
            Some(Msg::PreviewTheme("terminal".into()))
        );
        assert_eq!(press(&mut s, Key::Char('j')), None);
        assert_eq!(
            press(&mut s, Key::Char(' ')),
            Some(Msg::SelectTheme("terminal".into()))
        );
        assert_eq!(press(&mut s, Key::Enter), None);
        assert_eq!(press(&mut s, Key::Char('x')), Some(Msg::CancelTheme));
        assert_eq!(press(&mut s, Key::Esc), None);
    }

    // -- size and scrolling

    #[test]
    fn the_overlay_is_41_wide_and_five_rows_taller_than_the_list() {
        let size = selector().desired_size();
        assert_eq!(size, Size::new(41, 7 + 5));
    }

    #[test]
    fn a_long_label_widens_the_overlay_by_ten_columns() {
        let registry = registry_with(&[("a-very-long-theme-name-indeed-yes", false)]);
        let size = selector_for(&registry, "default").desired_size();
        let label = "a-very-long-theme-name-indeed-yes (invalid)";
        assert_eq!(
            usize::from(size.width),
            label.len() + 10,
            "the `(invalid)` text counts"
        );
        assert_eq!(size.height, 8 + 5);
    }

    #[test]
    fn the_rows_are_the_height_less_the_chrome() {
        assert_eq!(ThemeSelector::rows_in(12), 7);
        assert_eq!(ThemeSelector::rows_in(17), 12);
        assert_eq!(ThemeSelector::rows_in(5), 0);
        assert_eq!(ThemeSelector::rows_in(2), 0);
    }

    /// 12 themes in an overlay of 10 rows (5 visible): the list follows the
    /// cursor and never shows more than the rows it has.
    #[test]
    fn with_more_themes_than_rows_the_list_scrolls_to_keep_the_cursor_visible() {
        let mut s = numbered(12);
        let area = Rect::new(0, 0, 41, 10);
        s.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(5));
        let text = |s: &mut ThemeSelector| rows_of(&render(41, 10, |f| s.view(f, area)));
        let rows = text(&mut s);
        assert!(rows[2].contains("> Theme 0"), "{}", rows[2]);
        assert!(rows[6].contains("Theme 4"), "{}", rows[6]);
        assert!(!rows.iter().any(|r| r.contains("Theme 5")));
        for _ in 0..4 {
            press(&mut s, Key::Down);
        }
        let rows = text(&mut s);
        assert!(rows[6].contains("> Theme 4"), "the last row: {}", rows[6]);
        press(&mut s, Key::Down);
        let rows = text(&mut s);
        assert!(rows[2].contains("Theme 1"), "{}", rows[2]);
        assert!(rows[6].contains("> Theme 5"), "{}", rows[6]);
        for _ in 0..20 {
            press(&mut s, Key::Down);
        }
        let rows = text(&mut s);
        assert!(rows[2].contains("Theme 7"), "{}", rows[2]);
        assert!(rows[6].contains("> Theme 11"), "{}", rows[6]);
        // And back up: the list stays until the cursor reaches its top.
        press(&mut s, Key::Up);
        let rows = text(&mut s);
        assert!(rows[2].contains("Theme 7"), "{}", rows[2]);
        assert!(rows[5].contains("> Theme 10"), "{}", rows[5]);
        for _ in 0..4 {
            press(&mut s, Key::Up);
        }
        let rows = text(&mut s);
        assert!(rows[2].contains("> Theme 6"), "{}", rows[2]);
    }

    #[test]
    fn twelve_themes_fit_when_the_overlay_has_the_rows_for_them() {
        let mut s = numbered(12);
        let size = s.desired_size();
        assert_eq!(size.height, 17);
        let buffer = draw(&mut s);
        let rows = rows_of(&buffer);
        for i in 0..12 {
            assert!(
                rows.iter().any(|r| r.contains(&format!("Theme {i} "))),
                "Theme {i}"
            );
        }
    }

    #[test]
    fn announcing_the_rows_reveals_a_cursor_below_them() {
        let mut s = numbered(12);
        for _ in 0..8 {
            press(&mut s, Key::Down);
        }
        s.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(5));
        let rows = rows_of(&render(41, 10, |f| s.view(f, Rect::new(0, 0, 41, 10))));
        assert!(rows.iter().any(|r| r.contains("> Theme 8")), "{rows:?}");
    }

    // -- drawing

    #[test]
    fn the_overlay_reads_as_the_spec_shows_it() {
        let registry = ThemeRegistry::builtin();
        let mut s = selector_for(&registry, "terminal");
        let rows = rows_of(&draw(&mut s));
        let expected = [
            "╭─ Themes ──────────────────────────────╮",
            "│                                       │",
            "│    Default                            │",
            "│  > Terminal                           │",
            "│    Catppuccin                         │",
            "│    Gruvbox                            │",
            "│    Nord                               │",
            "│    Tokyo Night                        │",
            "│    Solarized                          │",
            "│                                       │",
            "│ ENTER to select | ESC to cancel       │",
            "╰───────────────────────────────────────╯",
        ];
        for (i, line) in expected.iter().enumerate() {
            let row: String = rows[2 + i].chars().skip(10).take(41).collect();
            assert_eq!(&row, line, "row {i}");
        }
    }

    #[test]
    fn the_help_row_follows_the_keymap() {
        let mut raw = crate::config::RawKeys::new();
        let overlay = raw.entry("overlay".to_owned()).or_default();
        overlay.insert("select".to_owned(), vec!["space".to_owned()]);
        overlay.insert("close".to_owned(), vec!["x".to_owned()]);
        let (keymap, _) = Keymap::from_config(&raw);
        let mut s = ThemeSelector::new(
            theme_items(&ThemeRegistry::builtin()),
            "default",
            builtin::default_theme(),
            keymap,
        );
        let rows = rows_of(&draw(&mut s));
        assert!(
            rows[12].contains("│ SPACE to select | x to cancel"),
            "{}",
            rows[12]
        );
    }

    #[test]
    fn the_colours_come_from_the_theme() {
        let theme = builtin::default_theme()
            .with_role(Role::Foreground, Color::Rgb(250, 1, 1))
            .with_role(Role::MutedForeground, Color::Rgb(2, 250, 2))
            .with_role(Role::Primary, Color::Rgb(3, 3, 250))
            .with_role(Role::Ring, Color::Rgb(5, 250, 250))
            .with_role(Role::Surface, Color::Rgb(9, 9, 9));
        let mut s = ThemeSelector::new(
            theme_items(&ThemeRegistry::builtin()),
            "terminal",
            theme.clone(),
            Keymap::default(),
        );
        let buffer = draw(&mut s);
        let at = |col: u16, row: u16| &buffer[(10 + col, 2 + row)];
        // Border and fill.
        assert_eq!(at(0, 0).fg, theme.role(Role::Ring));
        assert_eq!(at(0, 0).symbol(), "╭");
        assert_eq!(at(0, 5).fg, theme.role(Role::Ring));
        assert_eq!(at(20, 4).bg, theme.role(Role::Surface));
        // Title in foreground.
        assert_eq!(at(3, 0).symbol(), "T");
        assert_eq!(at(3, 0).fg, theme.role(Role::Foreground));
        // The cursor row: `>` in primary, the label bold foreground, not
        // inverted.
        let marker = at(3, 3);
        assert_eq!(marker.symbol(), ">");
        assert_eq!(marker.fg, theme.role(Role::Primary));
        assert_eq!(marker.bg, theme.role(Role::Surface));
        let label = at(5, 3);
        assert_eq!(label.symbol(), "T");
        assert_eq!(label.fg, theme.role(Role::Foreground));
        assert!(label.modifier.contains(Modifier::BOLD));
        assert!(!label.modifier.contains(Modifier::REVERSED));
        assert_eq!(at(12, 3).fg, theme.role(Role::Foreground));
        // Other rows are muted and not bold.
        let other = at(5, 2);
        assert_eq!(other.symbol(), "D");
        assert_eq!(other.fg, theme.role(Role::MutedForeground));
        assert!(!other.modifier.contains(Modifier::BOLD));
        assert_eq!(at(3, 2).symbol(), " ");
        // The help row.
        let key = at(2, 10);
        assert_eq!(key.symbol(), "E");
        assert_eq!(key.fg, theme.style(Token::HelpKey).fg.unwrap());
        let text = at(8, 10);
        assert_eq!(text.symbol(), "t");
        assert_eq!(text.fg, theme.style(Token::HelpText).fg.unwrap());
        assert_eq!(text.bg, theme.role(Role::Surface));
    }

    #[test]
    fn an_invalid_theme_is_muted() {
        let registry = registry_with(&[("my-theme", false)]);
        let theme = builtin::default_theme();
        let mut s = selector_for(&registry, "default");
        let buffer = draw(&mut s);
        let rows = rows_of(&buffer);
        let row = rows
            .iter()
            .position(|r| r.contains("my-theme (invalid)"))
            .unwrap();
        let col = u16::try_from(rows[row].find("my-theme").unwrap()).unwrap();
        let cell = &buffer[(col, u16::try_from(row).unwrap())];
        assert_eq!(cell.fg, theme.role(Role::MutedForeground));
        assert!(!cell.modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn a_transparent_theme_clears_but_does_not_fill() {
        let theme = builtin::default_theme().with_transparent_background(true);
        let mut s = ThemeSelector::new(
            theme_items(&ThemeRegistry::builtin()),
            "default",
            theme,
            Keymap::default(),
        );
        let area = Rect::new(2, 1, 41, 12);
        let buffer = render(50, 14, |frame| {
            frame.render_widget(
                ratatui::widgets::Paragraph::new(vec![
                    ratatui::text::Line::from("x".repeat(50));
                    14
                ]),
                frame.area(),
            );
            s.view(frame, area);
        });
        assert_eq!(buffer[(30, 9)].symbol(), " ", "the text under it is gone");
        assert_eq!(buffer[(30, 9)].bg, Color::Reset);
        assert_eq!(buffer[(2, 1)].symbol(), "╭");
        assert_eq!(buffer[(1, 1)].symbol(), "x", "and nothing outside it");
    }

    #[test]
    fn the_overlay_draws_in_a_small_and_a_degenerate_area() {
        let mut s = numbered(12);
        for (w, h) in [(0, 0), (1, 1), (2, 2), (5, 3), (41, 4), (41, 5), (41, 6)] {
            let _ = render(80, 24, |frame| s.view(frame, Rect::new(1, 1, w, h)));
        }
        // An area off the screen is cut to it.
        let _ = render(20, 10, |frame| s.view(frame, Rect::new(15, 8, 41, 12)));
    }

    // -- attributes

    #[test]
    fn the_theme_attribute_replaces_the_theme() {
        let mut s = selector();
        let other = builtin::default_theme().with_transparent_background(true);
        s.attr(THEME_ATTR, theme_value(&other));
        let value = s.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&value), Some(other));
    }

    #[test]
    fn the_rows_attribute_is_kept() {
        let mut s = selector();
        s.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(4));
        assert_eq!(
            s.query(VISIBLE_ROWS_ATTR).unwrap().into_attr(),
            AttrValue::Length(4)
        );
        assert!(s.query(Attribute::Focus).is_none());
    }

    // -- snapshots

    #[test]
    fn snapshot_builtins_text() {
        let mut s = selector();
        insta::assert_snapshot!(buffer_to_text(&draw(&mut s)));
    }

    #[test]
    fn snapshot_builtins_styled() {
        let mut s = selector();
        insta::assert_snapshot!(buffer_to_styled(&draw(&mut s)));
    }

    #[test]
    fn snapshot_with_an_invalid_user_theme_styled() {
        let registry = registry_with(&[("alpha", true), ("my-theme", false)]);
        let mut s = selector_for(&registry, "nord");
        insta::assert_snapshot!(buffer_to_styled(&draw(&mut s)));
    }

    #[test]
    fn snapshot_scrolled_text() {
        let mut s = numbered(12);
        for _ in 0..9 {
            press(&mut s, Key::Down);
        }
        s.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(5));
        let buffer = render(41, 10, |f| s.view(f, Rect::new(0, 0, 41, 10)));
        insta::assert_snapshot!(buffer_to_text(&buffer));
    }
}
