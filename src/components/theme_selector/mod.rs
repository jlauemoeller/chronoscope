//! The theme selector overlay (product §7.4): a small centred list of the
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
use tuirealm::event::{Event, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::{State, StateValue};
use unicode_width::UnicodeWidthStr;

pub use rows::{ThemeItem, ThemeRows};

use super::double_click::{CLICK_CLOCK_ATTR, ClickClock, LastPress};
use super::list_cursor::{ListCursor, VISIBLE_ROWS_ATTR};
use super::titled_block::titled_block;
use super::{HelpLine, Hint, THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, RoutedMouse, UserEvent};
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
pub fn theme_items(registry: &ThemeRegistry) -> ThemeItems {
    ThemeItems::new(
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
            .collect(),
    )
}

/// The themes the selector lists, in the order it lists them: its content,
/// which says how big the overlay wants to be before it is mounted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeItems {
    items: Vec<ThemeItem>,
}

impl ThemeItems {
    /// The list of `items`.
    pub fn new(items: Vec<ThemeItem>) -> Self {
        Self { items }
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
}

/// The help row: the hints from the keymap, so remapped keys show up, on
/// `surface`.
fn help_line(keymap: &Keymap, theme: &Theme) -> HelpLine {
    let hints = [(Action::Select, "to select"), (Action::Close, "to cancel")]
        .into_iter()
        .filter_map(|(action, text)| {
            let key = keymap.label(Context::Overlay, action)?;
            Some(Hint::new(key, text))
        })
        .collect();
    HelpLine::new(hints, Vec::new(), theme.clone(), Role::Surface).inset(HELP_INSET)
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
///
/// A left press on a theme moves the cursor there and says
/// `Msg::ChooseTheme`, with `keep` when it completed a double-click; the
/// clock that is measured with is set with
/// [`with_click_clock`](Self::with_click_clock) or [`CLICK_CLOCK_ATTR`].
#[derive(Debug, Clone)]
pub struct ThemeSelector {
    items: Vec<ThemeItem>,
    cursor: ListCursor,
    /// The rows the list showed last time it was announced; 0 while unknown.
    rows: usize,
    theme: Theme,
    keymap: Keymap,
    /// The help row, built when the theme changes.
    help: HelpLine,
    /// The last left press on a theme, for double-clicks.
    presses: LastPress<String>,
}

impl ThemeSelector {
    /// A selector that lists `items`, with the cursor on the one whose id is
    /// `active` (or the first that can be selected, if none is). Its size is
    /// [`ThemeItems::desired_size`].
    pub fn new(items: ThemeItems, active: &str, theme: Theme, keymap: Keymap) -> Self {
        let ThemeItems { items } = items;
        let start = items
            .iter()
            .position(|item| item.selectable && item.id == active)
            .or_else(|| items.iter().position(|item| item.selectable));
        let mut cursor = ListCursor::default();
        cursor.set(items.len(), start);
        let help = help_line(&keymap, &theme);
        Self {
            items,
            cursor,
            rows: 0,
            theme,
            keymap,
            help,
            presses: LastPress::default(),
        }
    }

    /// Replaces the clock double-clicks are measured with.
    #[must_use]
    pub fn with_click_clock(mut self, clock: ClickClock) -> Self {
        self.presses.set_clock(clock);
        self
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
        self.move_cursor(target);
        true
    }

    /// Puts the cursor on `index`, scrolling the least that shows it. On the
    /// first or last theme that can be selected the rows that cannot be (and
    /// are listed before or after it) are scrolled into view too, or they
    /// could never be seen.
    fn move_cursor(&mut self, index: usize) {
        let last = self.items.len().saturating_sub(1);
        if self.neighbour(index, true).is_none() {
            self.cursor.select(Some(last));
        }
        if self.neighbour(index, false).is_none() {
            self.cursor.select(Some(0));
        }
        self.cursor.select(Some(index));
    }

    fn block(&self) -> Block<'static> {
        let mut block = titled_block(&self.theme, "Themes", Role::Ring);
        if !self.theme.transparent_background() {
            block = block.style(Style::new().bg(self.theme.role(Role::Surface)));
        }
        block
    }

    fn set_theme(&mut self, theme: Theme) {
        self.help = help_line(&self.keymap, &theme);
        self.theme = theme;
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
            self.help.view(frame, help);
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
                    self.set_theme(theme);
                }
            }
            (VISIBLE_ROWS_ATTR, AttrValue::Length(rows)) => {
                self.rows = rows;
                self.cursor.set_visible(rows);
                if let Some(selected) = self.cursor.selected() {
                    self.move_cursor(selected);
                }
            }
            (CLICK_CLOCK_ATTR, value) => {
                if let Some(clock) = ClickClock::from_value(&value) {
                    self.presses.set_clock(clock);
                }
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
        let key = match ev {
            Event::Keyboard(key) => key,
            // Only what the router sent here, in the overlay's coordinates.
            Event::User(UserEvent::Pointer(mouse)) => return self.on_mouse(mouse),
            // A press outside gives the previous theme back, as `Esc` does.
            Event::User(UserEvent::PointerOutside) => return Some(Msg::CancelTheme),
            Event::User(UserEvent::PointerElsewhere) => {
                self.presses.forget();
                return None;
            }
            _ => return None,
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
    /// The wheel moves the cursor a theme like the arrow keys. A left press on
    /// a theme that can be selected moves the cursor there and says
    /// [`Msg::ChooseTheme`] (the `App` previews it, or keeps it when the press
    /// completed a double-click); anywhere else in the overlay it only ends a
    /// double-click under way.
    fn on_mouse(&mut self, mouse: &RoutedMouse) -> Option<Msg> {
        match mouse.kind {
            MouseEventKind::ScrollUp => self.moved(false),
            MouseEventKind::ScrollDown => self.moved(true),
            _ if mouse.is_left_press() => {
                let Some(index) = self.row_at(mouse) else {
                    self.presses.forget();
                    return None;
                };
                self.move_cursor(index);
                let id = self.items[index].id.clone();
                let keep = self.presses.press(id.clone());
                Some(Msg::ChooseTheme { id, keep })
            }
            _ => None,
        }
    }

    /// The theme row at the position of `mouse` (relative to the overlay),
    /// if it is a theme that can be selected: not the border, the blank rows,
    /// the help row or an empty row below the last theme.
    fn row_at(&self, mouse: &RoutedMouse) -> Option<usize> {
        let (width, height) = (mouse.area.width, mouse.area.height);
        // The inside of the border, as `view` places it.
        let inner = Rect::new(1, 1, width.saturating_sub(2), height.saturating_sub(2));
        let (list, _) = Self::parts(inner);
        let inside = (list.x..list.right()).contains(&mouse.column);
        let line = mouse.row.checked_sub(list.y)?;
        if !inside || line >= list.height {
            return None;
        }
        let index = self.cursor.offset_for(usize::from(list.height)) + usize::from(line);
        self.items.get(index)?.selectable.then_some(index)
    }

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
    use crate::test_support::TestClock;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::{Token, TokenOverride, builtin};
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

    /// The size the app gives `selector`: that of the items it lists.
    fn size_of(selector: &ThemeSelector) -> Size {
        ThemeItems::new(selector.items.clone()).desired_size()
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
        ThemeSelector::new(
            ThemeItems::new(items),
            "t0",
            builtin::default_theme(),
            Keymap::default(),
        )
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
        let size = size_of(selector);
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
            ThemeItems::new(vec![ThemeItem::invalid("a")]),
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
            ThemeItems::new(vec![ThemeItem::invalid("a")]),
            "a",
            builtin::default_theme(),
            Keymap::default(),
        );
        assert_eq!(press(&mut s, Key::Enter), None);
        assert_eq!(press(&mut s, Key::Down), None);
    }

    // -- the mouse

    fn mouse(s: &ThemeSelector, kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
        Event::User(UserEvent::Pointer(RoutedMouse {
            kind,
            column,
            row,
            area: size_of(s),
        }))
    }

    const PRESS: MouseEventKind = MouseEventKind::Down(tuirealm::event::MouseButton::Left);

    /// A press on list row `line` (0 is the first theme).
    fn click_line(s: &mut ThemeSelector, line: u16) -> Option<Msg> {
        s.on(&mouse(s, PRESS, 6, 2 + line))
    }

    fn chosen(id: &str, keep: bool) -> Option<Msg> {
        Some(Msg::ChooseTheme {
            id: id.into(),
            keep,
        })
    }

    #[test]
    fn a_click_on_a_theme_moves_the_cursor_there_and_says_which() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "default");
        // Rows are Default, Terminal, Catppuccin, Gruvbox, ...
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", false));
        assert_eq!(selected(&s).as_deref(), Some("gruvbox"));
        assert_eq!(click_line(&mut s, 0), chosen("default", false));
        assert_eq!(selected(&s).as_deref(), Some("default"));
    }

    #[test]
    fn a_click_on_the_theme_under_the_cursor_says_so_too() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "nord");
        assert_eq!(click_line(&mut s, 4), chosen("nord", false));
        assert_eq!(selected(&s).as_deref(), Some("nord"));
    }

    #[test]
    fn a_click_anywhere_but_on_a_theme_row_does_nothing() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "nord");
        let size = size_of(&s);
        let last = size.height - 1;
        // The top border, the blank row, the blank row under the list, the
        // help row and the bottom border; the border columns of a theme row.
        for row in [0, 1, last - 2, last - 1, last] {
            assert_eq!(s.on(&mouse(&s, PRESS, 6, row)), None, "row {row}");
        }
        for column in [0, size.width - 1] {
            assert_eq!(s.on(&mouse(&s, PRESS, column, 3)), None, "column {column}");
        }
        assert_eq!(selected(&s).as_deref(), Some("nord"));
    }

    #[test]
    fn a_click_below_the_last_theme_does_nothing() {
        // Two themes in an overlay with room for five.
        let items = vec![ThemeItem::valid("a", "A"), ThemeItem::valid("b", "B")];
        let mut s = ThemeSelector::new(
            ThemeItems::new(items),
            "a",
            builtin::default_theme(),
            Keymap::default(),
        );
        let event = Event::User(UserEvent::Pointer(RoutedMouse {
            kind: PRESS,
            column: 6,
            row: 5,
            area: Size::new(41, 10),
        }));
        assert_eq!(s.on(&event), None);
        assert_eq!(selected(&s).as_deref(), Some("a"));
    }

    #[test]
    fn a_click_on_an_invalid_theme_does_nothing() {
        let registry = registry_with(&[("broken", false)]);
        let mut s = selector_for(&registry, "default");
        assert_eq!(click_line(&mut s, 7), None);
        assert_eq!(selected(&s).as_deref(), Some("default"));
    }

    #[test]
    fn a_click_in_a_scrolled_list_hits_the_row_drawn_there() {
        let mut s = numbered(12);
        for _ in 0..9 {
            press(&mut s, Key::Down);
        }
        // Five rows fit; the cursor on Theme 9 scrolls the list to 5..10.
        s.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(5));
        let area = Size::new(41, 10);
        let at = |line: u16| {
            Event::User(UserEvent::Pointer(RoutedMouse {
                kind: PRESS,
                column: 6,
                row: 2 + line,
                area,
            }))
        };
        assert_eq!(s.on(&at(0)), chosen("t5", false));
        assert_eq!(s.on(&at(4)), chosen("t9", false));
        assert_eq!(selected(&s).as_deref(), Some("t9"));
    }

    #[test]
    fn a_press_outside_it_cancels_as_esc_does() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "default");
        click_line(&mut s, 3);
        let outside = Event::User(UserEvent::PointerOutside);
        assert_eq!(s.on(&outside), Some(Msg::CancelTheme));
        assert_eq!(press(&mut s, Key::Esc), Some(Msg::CancelTheme));
        assert_eq!(selected(&s).as_deref(), Some("gruvbox"), "the cursor stays");
    }

    // -- double-clicks

    /// The built-in themes, the cursor on `default`, double-clicks measured
    /// with the returned clock. Line 3 is Gruvbox, line 4 Nord.
    fn timed() -> (ThemeSelector, TestClock) {
        let clock = TestClock::new();
        let s = selector_for(&ThemeRegistry::builtin(), "default")
            .with_click_clock(clock.click_clock());
        (s, clock)
    }

    const ELSEWHERE: Event<UserEvent> = Event::User(UserEvent::PointerElsewhere);

    #[test]
    fn two_presses_on_a_theme_at_most_400_ms_apart_keep_it() {
        for (ms, keep) in [(0, true), (400, true), (401, false)] {
            let (mut s, clock) = timed();
            assert_eq!(click_line(&mut s, 3), chosen("gruvbox", false));
            clock.set_ms(ms);
            assert_eq!(click_line(&mut s, 3), chosen("gruvbox", keep), "{ms} ms");
        }
    }

    #[test]
    fn a_third_press_starts_a_new_double_click() {
        let (mut s, clock) = timed();
        click_line(&mut s, 3);
        clock.set_ms(100);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", true));
        clock.set_ms(200);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", false));
        clock.set_ms(300);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", true));
    }

    #[test]
    fn a_press_on_another_theme_ends_a_double_click() {
        let (mut s, clock) = timed();
        click_line(&mut s, 3);
        clock.set_ms(100);
        assert_eq!(click_line(&mut s, 4), chosen("nord", false));
        clock.set_ms(200);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", false));
    }

    #[test]
    fn a_press_not_on_a_theme_ends_a_double_click() {
        let size = size_of(&timed().0);
        // The top border, the blank row, the help row, a border column.
        for (column, row) in [(6, 0), (6, 1), (6, size.height - 2), (0, 5)] {
            let (mut s, clock) = timed();
            click_line(&mut s, 3);
            clock.set_ms(100);
            assert_eq!(s.on(&mouse(&s, PRESS, column, row)), None);
            clock.set_ms(200);
            assert_eq!(
                click_line(&mut s, 3),
                chosen("gruvbox", false),
                "{column}, {row}"
            );
        }
    }

    #[test]
    fn a_press_on_an_invalid_theme_ends_a_double_click() {
        let clock = TestClock::new();
        let registry = registry_with(&[("broken", false)]);
        let mut s = selector_for(&registry, "default").with_click_clock(clock.click_clock());
        click_line(&mut s, 3);
        assert_eq!(click_line(&mut s, 7), None);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", false));
    }

    #[test]
    fn a_press_elsewhere_ends_a_double_click_and_answers_nothing() {
        let (mut s, clock) = timed();
        click_line(&mut s, 3);
        clock.set_ms(100);
        assert_eq!(s.on(&ELSEWHERE), None);
        assert_eq!(selected(&s).as_deref(), Some("gruvbox"), "the cursor stays");
        clock.set_ms(200);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", false));
    }

    #[test]
    fn the_wheel_does_not_end_a_double_click() {
        let (mut s, clock) = timed();
        click_line(&mut s, 3);
        clock.set_ms(100);
        let down = mouse(&s, MouseEventKind::ScrollDown, 6, 5);
        assert_eq!(s.on(&down), Some(Msg::PreviewTheme("nord".into())));
        clock.set_ms(200);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", true));
    }

    #[test]
    fn the_click_clock_can_be_replaced_through_an_attribute() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "default");
        let clock = TestClock::new();
        s.attr(CLICK_CLOCK_ATTR, clock.click_clock().value());
        click_line(&mut s, 3);
        clock.set_ms(401);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", false));
    }

    #[test]
    fn the_system_clock_measures_double_clicks_by_default() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "default");
        click_line(&mut s, 3);
        assert_eq!(click_line(&mut s, 3), chosen("gruvbox", true));
    }

    #[test]
    fn the_wheel_moves_the_cursor_a_theme_and_previews_it() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "default");
        let down = mouse(&s, MouseEventKind::ScrollDown, 0, 0);
        let up = mouse(&s, MouseEventKind::ScrollUp, 0, 0);
        assert_eq!(s.on(&down), Some(Msg::PreviewTheme("terminal".into())));
        assert_eq!(s.on(&down), Some(Msg::PreviewTheme("catppuccin".into())));
        assert_eq!(s.on(&up), Some(Msg::PreviewTheme("terminal".into())));
        assert_eq!(selected(&s).as_deref(), Some("terminal"));
        assert_eq!(s.on(&up), Some(Msg::PreviewTheme("default".into())));
        assert_eq!(s.on(&up), None, "the first theme");
        for _ in 0..10 {
            s.on(&down);
        }
        assert_eq!(selected(&s).as_deref(), Some("solarized"));
        assert_eq!(s.on(&down), None, "the last theme");
    }

    #[test]
    fn other_mouse_events_and_the_raw_event_do_nothing() {
        let mut s = selector_for(&ThemeRegistry::builtin(), "nord");
        for kind in [
            MouseEventKind::Down(tuirealm::event::MouseButton::Right),
            MouseEventKind::Up(tuirealm::event::MouseButton::Left),
            MouseEventKind::Moved,
            MouseEventKind::ScrollLeft,
        ] {
            assert_eq!(s.on(&mouse(&s, kind, 6, 3)), None, "{kind:?}");
        }
        let raw = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::ScrollDown,
            modifiers: KeyModifiers::NONE,
            column: 6,
            row: 3,
        });
        assert_eq!(s.on(&raw), None);
        assert_eq!(selected(&s).as_deref(), Some("nord"));
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
        let size = theme_items(&ThemeRegistry::builtin()).desired_size();
        assert_eq!(size, Size::new(41, 7 + 5));
    }

    #[test]
    fn a_long_label_widens_the_overlay_by_ten_columns() {
        let registry = registry_with(&[("a-very-long-theme-name-indeed-yes", false)]);
        let size = theme_items(&registry).desired_size();
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
        let size = size_of(&s);
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

    // -- invalid themes at the ends of a scrolled list

    /// The built-ins, nine valid user themes named `User 01` ... and the
    /// user files `extra` (`(id, valid)`).
    fn long_registry(extra: &[(&str, bool)]) -> ThemeRegistry {
        let dir = tempfile::tempdir().unwrap();
        for i in 1..=9 {
            let text = theme_toml(&format!("User {i:02}"));
            std::fs::write(dir.path().join(format!("user-{i:02}.toml")), text).unwrap();
        }
        for (id, valid) in extra {
            let text = if *valid {
                theme_toml(id)
            } else {
                "name = \"broken\"\nforeground = \"not a colour\"\n".to_owned()
            };
            std::fs::write(dir.path().join(format!("{id}.toml")), text).unwrap();
        }
        ThemeRegistry::load(dir.path())
    }

    /// The overlay as the app places it at 60x20 (41x18 at (9, 1)), with the
    /// rows announced.
    fn draw_at_60x20(selector: &mut ThemeSelector) -> ratatui::buffer::Buffer {
        selector.attr(
            VISIBLE_ROWS_ATTR,
            AttrValue::Length(ThemeSelector::rows_in(18)),
        );
        render(60, 20, |frame| {
            selector.view(frame, Rect::new(9, 1, 41, 18))
        })
    }

    fn go_down(selector: &mut ThemeSelector, times: usize) {
        for _ in 0..times {
            press(selector, Key::Down);
        }
    }

    fn go_up(selector: &mut ThemeSelector, times: usize) {
        for _ in 0..times {
            press(selector, Key::Up);
        }
    }

    #[test]
    fn a_trailing_invalid_theme_is_scrolled_into_view_with_the_last_valid_one() {
        let registry = long_registry(&[("zz-broken", false)]);
        let mut s = selector_for(&registry, "default");
        go_down(&mut s, 30);
        assert_eq!(selected(&s).as_deref(), Some("user-09"));
        let rows = rows_of(&draw_at_60x20(&mut s));
        assert!(rows[14].contains("│  > User 09  "), "{}", rows[14]);
        assert!(
            rows[15].contains("│    zz-broken (invalid)  "),
            "{}",
            rows[15]
        );
        assert!(!rows.iter().any(|r| r.contains("Default")));
    }

    #[test]
    fn a_leading_invalid_theme_is_scrolled_back_into_view_with_the_first_valid_one() {
        let dir = tempfile::tempdir().unwrap();
        let broken = "name = \"broken\"\nforeground = \"not a colour\"\n";
        std::fs::write(dir.path().join("default.toml"), broken).unwrap();
        for i in 1..=9 {
            let text = theme_toml(&format!("User {i:02}"));
            std::fs::write(dir.path().join(format!("user-{i:02}.toml")), text).unwrap();
        }
        let registry = ThemeRegistry::load(dir.path());
        let mut s = selector_for(&registry, "terminal");
        assert_eq!(selected(&s).as_deref(), Some("terminal"));
        let open = rows_of(&draw_at_60x20(&mut s));
        assert!(open[3].contains("│    default (invalid)  "), "{}", open[3]);
        go_down(&mut s, 30);
        let end = rows_of(&draw_at_60x20(&mut s));
        assert!(!end.iter().any(|r| r.contains("default (invalid)")));
        go_up(&mut s, 30);
        assert_eq!(selected(&s).as_deref(), Some("terminal"));
        let back = rows_of(&draw_at_60x20(&mut s));
        assert!(back[3].contains("│    default (invalid)  "), "{}", back[3]);
        assert!(back[4].contains("│  > Terminal  "), "{}", back[4]);
    }

    #[test]
    fn the_invalid_rows_show_when_the_rows_are_announced_after_the_cursor_moved() {
        let registry = long_registry(&[("zz-broken", false)]);
        let mut s = selector_for(&registry, "user-09");
        let rows = rows_of(&draw_at_60x20(&mut s));
        assert!(rows[15].contains("zz-broken (invalid)"), "{}", rows[15]);
    }

    #[test]
    fn several_trailing_invalid_themes_show_when_they_fit() {
        let registry = long_registry(&[("zz-a", false), ("zz-b", false)]);
        let mut s = selector_for(&registry, "default");
        go_down(&mut s, 30);
        let rows = rows_of(&draw_at_60x20(&mut s));
        assert!(rows[14].contains("zz-a (invalid)"), "{}", rows[14]);
        assert!(rows[15].contains("zz-b (invalid)"), "{}", rows[15]);
        assert!(rows[13].contains("> User 09"), "{}", rows[13]);
    }

    #[test]
    fn a_long_run_of_invalid_themes_never_hides_the_cursor() {
        let items: Vec<ThemeItem> = std::iter::once(ThemeItem::valid("ok", "Ok"))
            .chain((0..20).map(|i| ThemeItem::invalid(format!("bad-{i}"))))
            .collect();
        let mut s = ThemeSelector::new(
            ThemeItems::new(items),
            "ok",
            builtin::default_theme(),
            Keymap::default(),
        );
        let rows = rows_of(&draw_at_60x20(&mut s));
        assert!(rows.iter().any(|r| r.contains("> Ok")), "{rows:?}");
    }

    #[test]
    fn a_label_wider_than_the_overlay_is_cut_with_an_ellipsis() {
        let long = "An Extremely Long User Theme Name That Goes On And On Forever And Ever";
        let items = vec![
            ThemeItem::valid("a", "Short"),
            ThemeItem::valid("b", long),
            ThemeItem::invalid("an-extremely-long-invalid-theme-id-that-goes-on-and-on"),
        ];
        let mut s = ThemeSelector::new(
            ThemeItems::new(items),
            "b",
            builtin::default_theme(),
            Keymap::default(),
        );
        // 41 columns: 39 inside, the label gets 34.
        let area = Rect::new(0, 0, 41, 8);
        s.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(3));
        let rows = rows_of(&render(41, 8, |f| s.view(f, area)));
        assert_eq!(rows[3], "│  > An Extremely Long User Theme Name… │");
        assert_eq!(rows[4], "│    an-extremely-long-inval… (invalid) │");
        assert!(rows[2].contains("│    Short  "));
    }

    #[test]
    fn an_invalid_row_in_a_narrow_overlay_keeps_the_suffix_or_falls_back_without_panicking() {
        let items = vec![
            ThemeItem::valid("a", "Alpha"),
            ThemeItem::invalid("some-long-invalid-id"),
        ];
        let mut s = ThemeSelector::new(
            ThemeItems::new(items),
            "a",
            builtin::default_theme(),
            Keymap::default(),
        );
        // 25 wide: 23 inside, 18 for the label: 8 for the id and the suffix.
        s.attr(VISIBLE_ROWS_ATTR, AttrValue::Length(2));
        let rows = rows_of(&render(25, 8, |f| s.view(f, Rect::new(0, 0, 25, 8))));
        assert_eq!(rows[3], "│    some-lo… (invalid) │");
        // Too narrow for the suffix: the whole label is cut, as for any row.
        for width in 0..16 {
            let rows = rows_of(&render(20, 8, |f| s.view(f, Rect::new(0, 0, width, 8))));
            assert_eq!(rows.len(), 8);
        }
        let rows = rows_of(&render(16, 8, |f| s.view(f, Rect::new(0, 0, 16, 8))));
        assert!(rows[3].starts_with("│    some"), "{}", rows[3]);
        assert!(rows[3].contains('…'), "{}", rows[3]);
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
    fn a_new_theme_rebuilds_the_help_row_in_its_colours() {
        let key = Color::Rgb(201, 2, 3);
        let text = Color::Rgb(4, 202, 6);
        let mut s = selector();
        let other = builtin::default_theme()
            .with_token(Token::HelpKey, TokenOverride::fg(key))
            .with_token(Token::HelpText, TokenOverride::fg(text));
        let help_cell = |buffer: &ratatui::buffer::Buffer, column: u16| {
            let cell = &buffer[(10 + column, 12)];
            (cell.symbol().to_owned(), cell.fg)
        };
        let before = draw(&mut s);
        assert_eq!(help_cell(&before, 2).0, "E");
        assert_ne!(help_cell(&before, 2).1, key);
        s.attr(THEME_ATTR, theme_value(&other));
        let after = draw(&mut s);
        // `ENTER` in `help_key`, ` to select` in `help_text`.
        assert_eq!(help_cell(&after, 2), ("E".to_owned(), key));
        assert_eq!(help_cell(&after, 8), ("t".to_owned(), text));
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

    /// The selector over `theme`, drawn by [`draw`], with the cursor on
    /// Terminal (row 3 of the overlay, the `>` in column 3).
    fn draw_terminal_selected(theme: &Theme) -> ratatui::buffer::Buffer {
        let mut s = ThemeSelector::new(
            theme_items(&ThemeRegistry::builtin()),
            "terminal",
            theme.clone(),
            Keymap::default(),
        );
        draw(&mut s)
    }

    #[test]
    fn the_marker_takes_an_overridden_selection_marker_token() {
        let red = Color::Rgb(255, 0, 0);
        let theme = builtin::default_theme()
            .with_role(Role::Surface, Color::Rgb(9, 9, 9))
            .with_token(Token::SelectionMarker, TokenOverride::fg(red));
        assert_ne!(theme.role(Role::Primary), red);
        let buffer = draw_terminal_selected(&theme);
        let marker = &buffer[(13, 5)];
        assert_eq!(marker.symbol(), ">");
        assert_eq!(marker.fg, red);
        // No background in the token: the overlay's fill shows through.
        assert_eq!(marker.bg, theme.role(Role::Surface));
        assert!(!marker.modifier.contains(Modifier::BOLD));
        assert!(!marker.modifier.contains(Modifier::REVERSED));
        // The label after it is still bold foreground, not inverted.
        let label = &buffer[(15, 5)];
        assert_eq!(label.symbol(), "T");
        assert_eq!(label.fg, theme.role(Role::Foreground));
        assert!(label.modifier.contains(Modifier::BOLD));
        assert!(!label.modifier.contains(Modifier::REVERSED));
        // Other rows stay muted.
        let other = &buffer[(15, 4)];
        assert_eq!(other.symbol(), "D");
        assert_eq!(other.fg, theme.role(Role::MutedForeground));
    }

    #[test]
    fn the_marker_takes_the_selection_marker_token_background_too() {
        let (fg, bg) = (Color::Rgb(255, 0, 0), Color::Rgb(0, 0, 255));
        let theme = builtin::default_theme()
            .with_token(Token::SelectionMarker, TokenOverride::fg_bg(fg, bg));
        let buffer = draw_terminal_selected(&theme);
        let marker = &buffer[(13, 5)];
        assert_eq!(marker.symbol(), ">");
        assert_eq!((marker.fg, marker.bg), (fg, bg));
        // Only the marker: the gap and the label keep the overlay's fill.
        assert_eq!(buffer[(14, 5)].bg, theme.role(Role::Surface));
        assert_eq!(buffer[(15, 5)].bg, theme.role(Role::Surface));
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
    fn snapshot_with_an_overridden_selection_marker_styled() {
        let theme = builtin::default_theme().with_token(
            Token::SelectionMarker,
            TokenOverride::fg(Color::Rgb(255, 0, 0)),
        );
        insta::assert_snapshot!(buffer_to_styled(&draw_terminal_selected(&theme)));
    }

    #[test]
    fn snapshot_with_an_invalid_user_theme_styled() {
        let registry = registry_with(&[("alpha", true), ("my-theme", false)]);
        let mut s = selector_for(&registry, "nord");
        insta::assert_snapshot!(buffer_to_styled(&draw(&mut s)));
    }

    #[test]
    fn snapshot_scrolled_with_a_trailing_invalid_theme_60x20_text() {
        let registry = long_registry(&[("zz-broken", false)]);
        let mut s = selector_for(&registry, "default");
        go_down(&mut s, 30);
        insta::assert_snapshot!(buffer_to_text(&draw_at_60x20(&mut s)));
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
