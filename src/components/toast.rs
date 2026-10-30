//! The toast: a small notification in the bottom-right corner that expires
//! on `Tick` (SPEC §4.2).

use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::Event;
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::{THEME_ATTR, theme_from_value, theme_value};
use crate::app::{Msg, UserEvent};
use crate::theme::{Theme, Token};

/// How long a toast stays on screen.
pub const TOAST_DURATION: Duration = Duration::from_secs(2);
/// A toast box is at most this wide, border included.
pub const MAX_TOAST_WIDTH: u16 = 60;
/// The columns a box adds to its text: two borders and one space of padding on
/// each side.
pub const TOAST_CHROME: u16 = 4;

/// Where "now" comes from. Tests inject their own.
type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// A 3-row rounded box in the `toast` style. It draws into the area it is
/// given (the `App` places it, see `layout`) and cuts text that is too long
/// for it with `…`.
///
/// It is subscribed to `Tick` and answers [`Msg::ToastExpired`] on the first
/// tick at or after `created_at + duration`.
#[derive(Clone)]
pub struct Toast {
    text: String,
    theme: Theme,
    created_at: Instant,
    duration: Duration,
    clock: Clock,
}

impl fmt::Debug for Toast {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Toast")
            .field("text", &self.text)
            .field("created_at", &self.created_at)
            .field("duration", &self.duration)
            .finish_non_exhaustive()
    }
}

impl Toast {
    /// A toast created at `created_at` that expires after `duration`, read
    /// against the system clock.
    pub fn new(
        text: impl Into<String>,
        theme: Theme,
        created_at: Instant,
        duration: Duration,
    ) -> Self {
        Self {
            text: text.into(),
            theme,
            created_at,
            duration,
            clock: Arc::new(Instant::now),
        }
    }

    /// Replaces the clock that decides when the toast has expired.
    #[must_use]
    pub fn with_clock(mut self, clock: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    /// Whether the toast has expired at `now`.
    pub fn expired_at(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.created_at) >= self.duration
    }

    /// The text cut to fit a box of `box_width` columns, with `…` where it
    /// was cut.
    fn fitted_text(&self, box_width: u16) -> String {
        let room = usize::from(box_width.saturating_sub(TOAST_CHROME));
        truncate_with_ellipsis(&self.text, room)
    }

    fn box_style(&self) -> ratatui::style::Style {
        let mut style = self.theme.style(Token::Toast);
        if self.theme.transparent_background() {
            style.bg = None;
        }
        style
    }
}

/// `text` cut to at most `width` columns, ending in `…` when it was cut.
pub fn truncate_with_ellipsis(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_owned();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = c.width().unwrap_or(0);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    if width > 0 {
        out.push('…');
    }
    out
}

impl Component for Toast {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let area = area.intersection(frame.area());
        if area.is_empty() {
            return;
        }
        let style = self.box_style();
        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .style(style);
        let inner = block.inner(area);
        frame.render_widget(Clear, area);
        frame.render_widget(block, area);
        let text = format!(" {} ", self.fitted_text(area.width));
        frame.render_widget(Paragraph::new(text).style(style), inner);
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
        (attr == THEME_ATTR).then(|| QueryResult::Owned(theme_value(&self.theme)))
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == THEME_ATTR
            && let Some(theme) = theme_from_value(&value)
        {
            self.theme = theme;
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for Toast {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Tick if self.expired_at((self.clock)()) => Some(Msg::ToastExpired),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_styled, buffer_to_text, render};
    use crate::theme::{Role, builtin};
    use std::sync::Mutex;

    fn toast(text: &str) -> Toast {
        Toast::new(
            text,
            builtin::default_theme(),
            Instant::now(),
            TOAST_DURATION,
        )
    }

    #[test]
    fn short_text_is_padded_by_one_space_in_a_rounded_box() {
        let mut component = toast("Config: unknown key \"x\"");
        let buffer = render(30, 3, |frame| component.view(frame, Rect::new(0, 0, 28, 3)));
        insta::assert_snapshot!(buffer_to_styled(&buffer));
    }

    #[test]
    fn long_text_is_truncated_with_an_ellipsis_at_80x24() {
        let mut component = toast(&format!("Config: {}", "very long warning ".repeat(10)));
        let area = Rect::new(80 - 1 - 60, 24 - 4, 60, 3);
        let buffer = render(80, 24, |frame| component.view(frame, area));
        let text = buffer_to_text(&buffer);
        insta::assert_snapshot!(text);
        let middle = text.lines().nth(21).unwrap();
        assert!(middle.trim_end().ends_with("… │"), "{middle:?}");
    }

    #[test]
    fn style_is_fg_foreground_on_bg_surface_everywhere_in_the_box() {
        let theme = builtin::default_theme();
        let mut component = toast("hi");
        let buffer = render(10, 3, |frame| component.view(frame, Rect::new(0, 0, 8, 3)));
        for y in 0..3 {
            for x in 0..8 {
                let cell = &buffer[(x, y)];
                assert_eq!(cell.fg, theme.role(Role::Foreground), "({x},{y})");
                assert_eq!(cell.bg, theme.role(Role::Surface), "({x},{y})");
            }
        }
        assert_eq!(buffer[(8, 1)].bg, ratatui::style::Color::Reset);
    }

    #[test]
    fn a_transparent_theme_paints_no_surface() {
        let mut component = Toast::new(
            "hi",
            builtin::default_theme().with_transparent_background(true),
            Instant::now(),
            TOAST_DURATION,
        );
        let buffer = render(10, 3, |frame| component.view(frame, Rect::new(0, 0, 8, 3)));
        for cell in buffer.content() {
            assert_eq!(cell.bg, ratatui::style::Color::Reset);
        }
        assert_eq!(buffer[(2, 1)].symbol(), "h");
    }

    #[test]
    fn truncation_keeps_the_result_within_the_width() {
        assert_eq!(truncate_with_ellipsis("hello", 5), "hello");
        assert_eq!(truncate_with_ellipsis("hello!", 5), "hell…");
        assert_eq!(truncate_with_ellipsis("日本語です", 6), "日本…");
        assert_eq!(truncate_with_ellipsis("hello", 0), "");
        assert_eq!(truncate_with_ellipsis("hello", 1), "…");
    }

    #[test]
    fn no_expiry_message_before_the_duration_has_elapsed() {
        let start = Instant::now();
        let now = Arc::new(Mutex::new(start));
        let clock = Arc::clone(&now);
        let mut component = Toast::new("x", builtin::default_theme(), start, TOAST_DURATION)
            .with_clock(move || *clock.lock().unwrap());
        assert_eq!(component.on(&Event::Tick), None);
        *now.lock().unwrap() = start + TOAST_DURATION - Duration::from_millis(1);
        assert_eq!(component.on(&Event::Tick), None);
    }

    #[test]
    fn the_expiry_message_comes_at_and_after_the_duration() {
        let start = Instant::now();
        let now = Arc::new(Mutex::new(start));
        let clock = Arc::clone(&now);
        let mut component = Toast::new("x", builtin::default_theme(), start, TOAST_DURATION)
            .with_clock(move || *clock.lock().unwrap());
        *now.lock().unwrap() = start + TOAST_DURATION;
        assert_eq!(component.on(&Event::Tick), Some(Msg::ToastExpired));
        *now.lock().unwrap() = start + TOAST_DURATION * 3;
        assert_eq!(component.on(&Event::Tick), Some(Msg::ToastExpired));
    }

    #[test]
    fn a_zero_duration_expires_on_the_first_tick() {
        let mut component = Toast::new(
            "x",
            builtin::default_theme(),
            Instant::now(),
            Duration::ZERO,
        );
        assert_eq!(component.on(&Event::Tick), Some(Msg::ToastExpired));
    }

    #[test]
    fn other_events_never_expire_the_toast() {
        let mut component = Toast::new(
            "x",
            builtin::default_theme(),
            Instant::now(),
            Duration::ZERO,
        );
        for event in [Event::WindowResize(1, 1), Event::FocusGained, Event::None] {
            assert_eq!(component.on(&event), None);
        }
    }

    #[test]
    fn degenerate_areas_do_not_panic() {
        let mut component = toast("hello");
        let _ = render(10, 3, |frame| {
            component.view(frame, Rect::new(0, 0, 0, 0));
            component.view(frame, Rect::new(0, 0, 3, 1));
            component.view(frame, Rect::new(5, 0, 50, 3));
        });
    }

    #[test]
    fn the_theme_can_be_replaced_through_an_attribute() {
        let mut component = toast("x");
        let other = builtin::terminal_theme();
        component.attr(THEME_ATTR, theme_value(&other));
        let queried = component.query(THEME_ATTR).unwrap().into_attr();
        assert_eq!(theme_from_value(&queried), Some(other));
        assert_eq!(component.state(), State::None);
        assert_eq!(component.perform(Cmd::None), CmdResult::NoChange);
    }
}
