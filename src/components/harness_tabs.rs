//! The harness tabs of the session selector (product §3.1, §5.6, §7.2).

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use unicode_width::UnicodeWidthStr;

use super::{THEME_ATTR, payload_from_value, payload_value, theme_from_value, theme_value};
use crate::app::{Msg, RoutedMouse, SelectorMsg, UserEvent};
use crate::model::HarnessId;
use crate::theme::{Role, Theme};

/// The attribute that sets the tabs ([`Tabs`]).
pub const TABS_ATTR: Attribute = Attribute::Custom("tabs");

/// What separates two tabs.
const SEPARATOR: &str = "  ";
/// The character the selected tab is underlined with.
const UNDERLINE: &str = "━";

/// One tab: a registered harness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tab {
    pub id: HarnessId,
    /// Whether the harness has data on this machine. Tabs of harnesses
    /// without are drawn like unselected ones and cannot be selected.
    pub available: bool,
}

/// The tabs, in order, and which one is selected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tabs {
    pub tabs: Vec<Tab>,
    pub selected: Option<HarnessId>,
}

/// Draws `[N] Name` for each harness on the first row of its area and, on
/// the second row, `━` in `primary` under exactly the selected tab's label.
/// The selected tab is `primary`; the others, available or not, are
/// `muted_foreground`. It never takes focus.
#[derive(Debug, Clone)]
pub struct HarnessTabs {
    theme: Theme,
    tabs: Tabs,
}

impl HarnessTabs {
    /// A bar without tabs.
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            tabs: Tabs::default(),
        }
    }

    /// The label of a tab: the number shown next to its name.
    fn label(id: HarnessId) -> String {
        format!("[{}] {}", id.number(), id.display_name())
    }

    fn label_line(&self) -> Line<'static> {
        let primary = Style::new().fg(self.theme.role(Role::Primary));
        let muted = Style::new().fg(self.theme.role(Role::MutedForeground));
        let mut spans = Vec::new();
        for (i, tab) in self.tabs.tabs.iter().enumerate() {
            if i > 0 {
                spans.push(Span::raw(SEPARATOR));
            }
            let selected = self.tabs.selected == Some(tab.id);
            let style = if selected { primary } else { muted };
            spans.push(Span::styled(Self::label(tab.id), style));
        }
        Line::from(spans)
    }

    /// The tab whose label is under `column` of the labels' row.
    fn tab_at(&self, column: u16) -> Option<&Tab> {
        let mut start = 0;
        for tab in &self.tabs.tabs {
            let end = start + Self::label(tab.id).width();
            if (start..end).contains(&usize::from(column)) {
                return Some(tab);
            }
            start = end + SEPARATOR.width();
        }
        None
    }

    /// A left press on the label of an available tab asks to switch to it.
    /// The separators, the underline row, and the labels of tabs without data
    /// do nothing.
    fn on_mouse(&self, mouse: &RoutedMouse) -> Option<Msg> {
        if !mouse.is_left_press() || mouse.row != 0 {
            return None;
        }
        self.tab_at(mouse.column)
            .filter(|tab| tab.available)
            .map(|tab| Msg::Selector(SelectorMsg::SwitchHarness(tab.id)))
    }

    /// The underline row: spaces up to the selected tab, then `━` as wide as
    /// its label. Empty when no tab is selected.
    fn underline_line(&self) -> Line<'static> {
        let mut offset = 0;
        for tab in &self.tabs.tabs {
            let width = Self::label(tab.id).width();
            if self.tabs.selected == Some(tab.id) {
                let primary = Style::new().fg(self.theme.role(Role::Primary));
                return Line::from(vec![
                    Span::raw(" ".repeat(offset)),
                    Span::styled(UNDERLINE.repeat(width), primary),
                ]);
            }
            offset += width + SEPARATOR.len();
        }
        Line::default()
    }
}

impl Component for HarnessTabs {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        let labels = Rect { height: 1, ..area };
        frame.render_widget(Paragraph::new(self.label_line()), labels);
        if area.height > 1 {
            let underline = Rect {
                y: area.y + 1,
                height: 1,
                ..area
            };
            frame.render_widget(Paragraph::new(self.underline_line()), underline);
        }
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            TABS_ATTR => Some(QueryResult::Owned(payload_value(self.tabs.clone()))),
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
            TABS_ATTR => {
                if let Some(tabs) = payload_from_value(&value) {
                    self.tabs = tabs;
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

impl AppComponent<Msg, UserEvent> for HarnessTabs {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::User(UserEvent::Mouse(mouse)) => self.on_mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;

    fn tabs(selected: Option<HarnessId>) -> Tabs {
        Tabs {
            tabs: vec![
                Tab {
                    id: HarnessId::Claude,
                    available: true,
                },
                // A fake second harness without data on this machine.
                Tab {
                    id: HarnessId::OpenCode,
                    available: false,
                },
            ],
            selected,
        }
    }

    fn bar(tabs: Tabs) -> HarnessTabs {
        let mut bar = HarnessTabs::new(builtin::default_theme());
        bar.attr(TABS_ATTR, payload_value(tabs));
        bar
    }

    fn draw(bar: &mut HarnessTabs) -> ratatui::buffer::Buffer {
        render(60, 2, |frame| bar.view(frame, frame.area()))
    }

    #[test]
    fn one_selected_and_one_muted_tab_snapshot() {
        let buffer = draw(&mut bar(tabs(Some(HarnessId::Claude))));
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn labels_are_numbered_and_separated_by_two_spaces() {
        let text = buffer_to_text(&draw(&mut bar(tabs(Some(HarnessId::Claude)))));
        assert!(text.starts_with("[1] Claude  [2] OpenCode"), "{text}");
    }

    #[test]
    fn the_underline_is_as_wide_as_the_selected_label() {
        let text = buffer_to_text(&draw(&mut bar(tabs(Some(HarnessId::Claude)))));
        assert_eq!(text.lines().nth(1).unwrap().trim_end(), "━".repeat(10));
    }

    #[test]
    fn the_underline_follows_the_selected_tab() {
        let mut two = tabs(Some(HarnessId::OpenCode));
        two.tabs[1].available = true;
        let text = buffer_to_text(&draw(&mut bar(two)));
        assert_eq!(
            text.lines().nth(1).unwrap().trim_end(),
            format!("{}{}", " ".repeat(12), "━".repeat(12))
        );
    }

    #[test]
    fn selected_is_primary_and_the_others_are_muted() {
        let theme = builtin::default_theme();
        let buffer = draw(&mut bar(tabs(Some(HarnessId::Claude))));
        assert_eq!(buffer[(0, 0)].fg, theme.role(Role::Primary));
        assert_eq!(buffer[(9, 0)].fg, theme.role(Role::Primary));
        assert_eq!(buffer[(12, 0)].fg, theme.role(Role::MutedForeground));
        assert_eq!(buffer[(0, 1)].fg, theme.role(Role::Primary));
    }

    #[test]
    fn without_a_selection_every_tab_is_muted_and_nothing_is_underlined() {
        let theme = builtin::default_theme();
        let buffer = draw(&mut bar(tabs(None)));
        assert_eq!(buffer[(0, 0)].fg, theme.role(Role::MutedForeground));
        assert_eq!(buffer_to_text(&buffer).lines().nth(1).unwrap().trim(), "");
    }

    #[test]
    fn tabs_are_queryable_and_the_theme_can_be_replaced() {
        let mut b = bar(tabs(Some(HarnessId::Claude)));
        let got: Tabs = payload_from_value(&b.query(TABS_ATTR).unwrap().into_attr()).unwrap();
        assert_eq!(got, tabs(Some(HarnessId::Claude)));
        let other = builtin::terminal_theme();
        b.attr(THEME_ATTR, theme_value(&other));
        assert_eq!(
            theme_from_value(&b.query(THEME_ATTR).unwrap().into_attr()),
            Some(other)
        );
    }

    #[test]
    fn it_ignores_events_and_never_takes_focus() {
        let mut b = bar(tabs(None));
        assert_eq!(b.on(&Event::Tick), None);
        assert_eq!(b.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(b.state(), State::None);
    }

    #[test]
    fn a_one_row_area_draws_only_the_labels() {
        let buffer = render(30, 1, |frame| {
            bar(tabs(Some(HarnessId::Claude))).view(frame, frame.area())
        });
        assert!(buffer_to_text(&buffer).starts_with("[1] Claude"));
    }

    // -- the mouse

    use ratatui::layout::Size;
    use tuirealm::event::{MouseButton, MouseEventKind};

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
        Event::User(UserEvent::Mouse(RoutedMouse {
            kind,
            column,
            row,
            area: Size::new(60, 2),
        }))
    }

    fn press(column: u16, row: u16) -> Event<UserEvent> {
        mouse(MouseEventKind::Down(MouseButton::Left), column, row)
    }

    fn switch(harness: HarnessId) -> Option<Msg> {
        Some(Msg::Selector(SelectorMsg::SwitchHarness(harness)))
    }

    #[test]
    fn a_press_on_the_label_of_an_available_tab_asks_to_switch_to_it() {
        let mut b = bar(tabs(None));
        // `[1] Claude` is columns 0..10.
        for column in [0, 4, 9] {
            assert_eq!(b.on(&press(column, 0)), switch(HarnessId::Claude));
        }
    }

    #[test]
    fn a_press_on_the_label_of_a_muted_tab_does_nothing() {
        let mut b = bar(tabs(Some(HarnessId::Claude)));
        // `[2] OpenCode` is columns 12..24 and has no data.
        for column in [12, 18, 23] {
            assert_eq!(b.on(&press(column, 0)), None, "{column}");
        }
    }

    #[test]
    fn a_press_between_the_labels_or_elsewhere_does_nothing() {
        let mut b = bar(tabs(Some(HarnessId::Claude)));
        for (column, row) in [(10, 0), (11, 0), (24, 0), (50, 0), (3, 1)] {
            assert_eq!(b.on(&press(column, row)), None, "{column}, {row}");
        }
    }

    #[test]
    fn tabs_ignore_every_other_mouse_event() {
        let mut b = bar(tabs(None));
        for kind in [
            MouseEventKind::Down(MouseButton::Right),
            MouseEventKind::Up(MouseButton::Left),
            MouseEventKind::ScrollDown,
            MouseEventKind::ScrollUp,
            MouseEventKind::Moved,
        ] {
            assert_eq!(b.on(&mouse(kind, 2, 0)), None, "{kind:?}");
        }
        // The raw event, in screen coordinates, is not theirs to read.
        let raw = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            modifiers: tuirealm::event::KeyModifiers::NONE,
            column: 2,
            row: 0,
        });
        assert_eq!(b.on(&raw), None);
    }
}
