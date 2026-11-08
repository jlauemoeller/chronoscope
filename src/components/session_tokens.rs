//! The Session Tokens panel of the session selector (product §3.2, §3.3): the
//! selected session's token counts, or an empty block.

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::list_pane::titled_block;
use super::panel_row::PanelRows;
use super::{THEME_ATTR, payload_from_value, payload_value, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::model::TokenUsage;
use crate::theme::{Role, Theme};
use crate::tokens::format_tokens;

/// The attribute that sets the token counts shown: a payload of
/// `Option<TokenUsage>`, `None` for the empty block.
pub const SESSION_TOKENS_ATTR: Attribute = Attribute::Custom("session_tokens");

/// The title of the panel.
const TITLE: &str = "Session Tokens";

/// A rounded box in `border` titled `Session Tokens`, below the Session Time
/// panel, with a row for each count, or no rows while it has no counts. It
/// never takes focus and ignores every event.
#[derive(Debug, Clone)]
pub struct SessionTokens {
    theme: Theme,
    tokens: Option<TokenUsage>,
}

impl SessionTokens {
    /// A panel showing `tokens`, or an empty one for `None`.
    pub fn new(tokens: Option<TokenUsage>, theme: Theme) -> Self {
        Self { theme, tokens }
    }

    /// The `(label, value)` rows, in order.
    fn rows(tokens: &TokenUsage) -> Vec<(&'static str, String)> {
        vec![
            ("Input", format_tokens(tokens.input)),
            ("Output", format_tokens(tokens.output)),
            ("Thinking", format_tokens(tokens.reasoning)),
            ("Cache read", format_tokens(tokens.cache_read)),
            ("Cache write", format_tokens(tokens.cache_write)),
        ]
    }
}

impl Component for SessionTokens {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        frame.render_widget(titled_block(&self.theme, TITLE, Role::Border), area);
        if let Some(tokens) = &self.tokens {
            frame.render_widget(PanelRows::new(Self::rows(tokens), &self.theme), area);
        }
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        match attr {
            THEME_ATTR => Some(QueryResult::Owned(theme_value(&self.theme))),
            SESSION_TOKENS_ATTR => Some(QueryResult::Owned(payload_value(self.tokens))),
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
            SESSION_TOKENS_ATTR => {
                if let Some(tokens) = payload_from_value::<Option<TokenUsage>>(&value) {
                    self.tokens = tokens;
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

impl AppComponent<Msg, UserEvent> for SessionTokens {
    fn on(&mut self, _ev: &Event<UserEvent>) -> Option<Msg> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::builtin;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};

    const HEIGHT: u16 = 7;

    fn draw(width: u16, theme: Theme) -> ratatui::buffer::Buffer {
        draw_tokens(None, width, theme)
    }

    fn draw_tokens(
        tokens: Option<TokenUsage>,
        width: u16,
        theme: Theme,
    ) -> ratatui::buffer::Buffer {
        let mut component = SessionTokens::new(tokens, theme);
        render(width, HEIGHT, |frame| component.view(frame, frame.area()))
    }

    /// The worked example.
    fn example() -> Option<TokenUsage> {
        Some(TokenUsage {
            input: 15_800,
            output: 6_300_000,
            reasoning: 2_200_000,
            cache_read: 862_300_000,
            cache_write: 30_700_000,
        })
    }

    #[test]
    fn with_tokens_default_theme_at_width_31_snapshot() {
        let buffer = draw_tokens(example(), 31, builtin::default_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn with_tokens_default_theme_at_width_75_snapshot() {
        let buffer = draw_tokens(example(), 75, builtin::default_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn with_tokens_terminal_theme_at_width_31_snapshot() {
        let buffer = draw_tokens(example(), 31, builtin::terminal_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn with_tokens_terminal_theme_at_width_75_snapshot() {
        let buffer = draw_tokens(example(), 75, builtin::terminal_theme());
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn a_narrow_panel_cuts_the_label_and_keeps_the_value() {
        // 17 columns: a row has 13 cells. `Cache write` and `30.7M` leave 7
        // for the label; `Cache read` and `862.3M` leave 6.
        let text = buffer_to_text(&draw_tokens(example(), 17, builtin::default_theme()));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[1], "│ Input   15.8k │");
        assert_eq!(lines[4], "│ Cache… 862.3M │");
        assert_eq!(lines[5], "│ Cache … 30.7M │");
        insta::assert_snapshot!(text);
    }

    #[test]
    fn the_worked_example_reads_as_in_the_wireframe() {
        let text = buffer_to_text(&draw_tokens(example(), 56, builtin::default_theme()));
        let lines: Vec<&str> = text.lines().collect();
        let row = |label: &str, value: &str| {
            format!(
                "│ {label}{}{value} │",
                " ".repeat(52 - label.len() - value.len())
            )
        };
        assert_eq!(lines[1], row("Input", "15.8k"));
        assert_eq!(lines[2], row("Output", "6.3M"));
        assert_eq!(lines[3], row("Thinking", "2.2M"));
        assert_eq!(lines[4], row("Cache read", "862.3M"));
        assert_eq!(lines[5], row("Cache write", "30.7M"));
    }

    #[test]
    fn a_count_no_message_reported_is_zero() {
        let text = buffer_to_text(&draw_tokens(
            Some(TokenUsage::default()),
            31,
            builtin::default_theme(),
        ));
        let lines: Vec<&str> = text.lines().collect();
        for (line, label) in
            lines[1..6]
                .iter()
                .zip(["Input", "Output", "Thinking", "Cache read", "Cache write"])
        {
            assert!(line.starts_with(&format!("│ {label}")), "{line}");
            assert!(line.ends_with(" 0 │"), "{line}");
        }
    }

    #[test]
    fn every_label_and_value_fits_at_the_narrowest_width() {
        // 60 columns: the panel is 31 wide and a row has 27 cells.
        let text = buffer_to_text(&draw_tokens(example(), 31, builtin::default_theme()));
        assert!(!text.contains('…'), "{text}");
        for line in text.lines().skip(1).take(5) {
            assert!(line.contains("  "), "{line}");
        }
        // A million billion tokens of each kind still leaves every label.
        let big = 1_000_000_000_000_000;
        let largest = TokenUsage {
            input: big,
            output: big,
            reasoning: big,
            cache_read: big,
            cache_write: big,
        };
        let text = buffer_to_text(&draw_tokens(Some(largest), 31, builtin::default_theme()));
        assert!(!text.contains('…'), "{text}");
    }

    #[test]
    fn labels_are_muted_and_values_foreground() {
        let theme = builtin::default_theme();
        assert_ne!(
            theme.role(Role::MutedForeground),
            theme.role(Role::Foreground)
        );
        let buffer = draw_tokens(example(), 31, theme.clone());
        for y in 1..=5 {
            for x in 2..7 {
                assert_eq!(
                    buffer[(x, y)].fg,
                    theme.role(Role::MutedForeground),
                    "{x},{y}"
                );
            }
            for x in 25..29 {
                assert_eq!(buffer[(x, y)].fg, theme.role(Role::Foreground), "{x},{y}");
            }
            assert_eq!(buffer[(0, y)].fg, theme.role(Role::Border));
            assert_eq!(buffer[(30, y)].fg, theme.role(Role::Border));
        }
    }

    #[test]
    fn the_tokens_are_set_through_an_attribute_and_none_clears_them() {
        let mut component = SessionTokens::new(None, builtin::default_theme());
        let query = |c: &SessionTokens| {
            payload_from_value::<Option<TokenUsage>>(
                &c.query(SESSION_TOKENS_ATTR).unwrap().into_attr(),
            )
            .unwrap()
        };
        assert_eq!(query(&component), None);
        component.attr(SESSION_TOKENS_ATTR, payload_value(example()));
        assert_eq!(query(&component), example());
        let buffer = render(31, HEIGHT, |frame| component.view(frame, frame.area()));
        assert!(buffer_to_text(&buffer).contains("862.3M"));
        component.attr(SESSION_TOKENS_ATTR, payload_value(None::<TokenUsage>));
        assert_eq!(query(&component), None);
        let buffer = render(31, HEIGHT, |frame| component.view(frame, frame.area()));
        assert!(!buffer_to_text(&buffer).contains("Input"));
    }

    #[test]
    fn default_theme_at_width_31_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(31, builtin::default_theme())));
    }

    #[test]
    fn default_theme_at_width_75_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(75, builtin::default_theme())));
    }

    #[test]
    fn terminal_theme_at_width_31_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(31, builtin::terminal_theme())));
    }

    #[test]
    fn terminal_theme_at_width_75_snapshot() {
        insta::assert_snapshot!(buffer_to_styled(&draw(75, builtin::terminal_theme())));
    }

    #[test]
    fn it_is_an_empty_rounded_box_with_its_title() {
        let text = buffer_to_text(&draw(31, builtin::default_theme()));
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), usize::from(HEIGHT));
        assert_eq!(
            lines[0],
            format!("╭─ {TITLE} {}╮", "─".repeat(31 - 5 - TITLE.len()))
        );
        for line in &lines[1..lines.len() - 1] {
            assert_eq!(*line, format!("│{}│", " ".repeat(29)));
        }
        assert_eq!(lines[lines.len() - 1], format!("╰{}╯", "─".repeat(29)));
    }

    #[test]
    fn the_border_is_border_and_the_title_is_foreground() {
        let theme = builtin::default_theme();
        let buffer = draw(31, theme.clone());
        assert_eq!(buffer[(0, 0)].fg, theme.role(Role::Border));
        assert_eq!(buffer[(0, 1)].fg, theme.role(Role::Border));
        assert_eq!(buffer[(30, HEIGHT - 1)].fg, theme.role(Role::Border));
        assert_eq!(buffer[(3, 0)].fg, theme.role(Role::Foreground));
        assert_ne!(theme.role(Role::Border), theme.role(Role::Ring));
    }

    #[test]
    fn the_theme_is_set_through_an_attribute() {
        let mut component = SessionTokens::new(example(), builtin::default_theme());
        let nord = builtin::get("nord").expect("nord is built in");
        component.attr(THEME_ATTR, theme_value(&nord));
        let queried = component.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&queried), Some(nord.clone()));
        let buffer = render(31, HEIGHT, |frame| component.view(frame, frame.area()));
        assert_eq!(buffer[(0, 0)].fg, nord.role(Role::Border));
    }

    #[test]
    fn it_ignores_events_and_a_tiny_area_does_not_panic() {
        let mut component = SessionTokens::new(example(), builtin::default_theme());
        assert_eq!(component.on(&Event::Tick), None);
        let key = Event::Keyboard(KeyEvent::new(Key::Enter, KeyModifiers::NONE));
        assert_eq!(component.on(&key), None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
        assert_eq!(component.state(), State::None);
        for (w, h) in [
            (0, 0),
            (1, 1),
            (2, 2),
            (5, 1),
            (31, 1),
            (3, HEIGHT),
            (31, 40),
        ] {
            let _ = render(31, 8, |frame| component.view(frame, Rect::new(0, 0, w, h)));
        }
    }
}
