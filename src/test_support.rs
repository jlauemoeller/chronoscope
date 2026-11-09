//! Helpers for rendering widgets into a Ratatui `TestBackend` and turning the
//! resulting buffer into text that `insta` can snapshot, and a clock tests
//! can set.

use std::fmt::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Modifier};
use unicode_width::UnicodeWidthStr;

use crate::components::ClickClock;

/// Draws into a `width` x `height` `TestBackend` terminal and returns the
/// resulting buffer.
pub fn render(width: u16, height: u16, draw: impl FnOnce(&mut Frame)) -> Buffer {
    let mut terminal =
        Terminal::new(TestBackend::new(width, height)).expect("TestBackend never fails");
    terminal
        .draw(draw)
        .expect("drawing to a TestBackend never fails");
    terminal.backend().buffer().clone()
}

/// Returns the buffer text, one line per row, trailing spaces kept.
pub fn buffer_to_text(buffer: &Buffer) -> String {
    let width = usize::from(buffer.area.width);
    let mut out = String::new();
    for row in buffer.content().chunks(width.max(1)) {
        let mut skip = 0;
        for cell in row {
            if skip > 0 {
                skip -= 1;
                continue;
            }
            let symbol = cell.symbol();
            out.push_str(symbol);
            skip = symbol.width().saturating_sub(1);
        }
        out.push('\n');
    }
    out
}

/// Returns the buffer text followed by the styles of every row.
///
/// Each row is written as `y: x0..x1 fg/bg/modifiers | ...`, one entry per
/// run of horizontally adjacent cells with identical style (`x1` exclusive).
/// Modifiers are `NONE` or the flag names joined with ` | `.
///
/// The continuation cells that follow a wide glyph take the style of the glyph
/// itself (Ratatui resets them), so a styled run of wide characters stays one
/// run. Underline colour is deliberately left out of the style.
pub fn buffer_to_styled(buffer: &Buffer) -> String {
    let mut out = buffer_to_text(buffer);
    out.push_str("--- styles (fg/bg/modifiers) ---\n");
    let width = usize::from(buffer.area.width);
    for (y, row) in buffer.content().chunks(width.max(1)).enumerate() {
        // Each glyph with the number of columns it covers.
        let mut glyphs: Vec<(usize, (Color, Color, Modifier))> = Vec::new();
        let mut x = 0;
        while x < row.len() {
            let cell = &row[x];
            let cols = cell.symbol().width().max(1).min(row.len() - x);
            glyphs.push((cols, style_key(cell)));
            x += cols;
        }
        let mut runs: Vec<String> = Vec::new();
        let mut start = 0;
        let mut i = 0;
        while i < glyphs.len() {
            let key = glyphs[i].1;
            let mut end = start;
            while i < glyphs.len() && glyphs[i].1 == key {
                end += glyphs[i].0;
                i += 1;
            }
            let (fg, bg, modifier) = key;
            runs.push(format!(
                "{start}..{end} {fg:?}/{bg:?}/{}",
                modifiers(modifier)
            ));
            start = end;
        }
        writeln!(out, "{y}: {}", runs.join(" | ")).expect("writing to a String never fails");
    }
    out
}

fn style_key(cell: &ratatui::buffer::Cell) -> (Color, Color, Modifier) {
    (cell.fg, cell.bg, cell.modifier)
}

fn modifiers(modifier: Modifier) -> String {
    if modifier.is_empty() {
        "NONE".to_string()
    } else {
        format!("{modifier:?}")
    }
}

/// A clock for tests that shows a fixed start plus the time it is
/// [`set`](Self::set) to, shared by its clones: for the double-click clock
/// of a component ([`click_clock`](Self::click_clock)) or of the `App`
/// ([`reader`](Self::reader)).
#[derive(Debug, Clone)]
pub struct TestClock {
    start: Instant,
    elapsed: Arc<Mutex<Duration>>,
}

impl Default for TestClock {
    fn default() -> Self {
        Self::new()
    }
}

impl TestClock {
    /// A clock that shows its start until it is set.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            elapsed: Arc::default(),
        }
    }

    /// Moves the clock to `elapsed` after its start.
    pub fn set(&self, elapsed: Duration) {
        *self.elapsed.lock().unwrap() = elapsed;
    }

    /// Moves the clock to `ms` milliseconds after its start.
    pub fn set_ms(&self, ms: u64) {
        self.set(Duration::from_millis(ms));
    }

    /// The time it shows.
    pub fn now(&self) -> Instant {
        self.start + *self.elapsed.lock().unwrap()
    }

    /// A function that reads this clock, for `App::set_click_clock`.
    pub fn reader(&self) -> impl Fn() -> Instant + Send + Sync + 'static {
        let clock = self.clone();
        move || clock.now()
    }

    /// A [`ClickClock`] that reads this clock.
    pub fn click_clock(&self) -> ClickClock {
        ClickClock::new(self.reader())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;
    use ratatui::style::Stylize;
    use ratatui::widgets::Paragraph;

    #[test]
    fn render_returns_buffer_of_requested_size() {
        let buffer = render(7, 3, |frame| {
            frame.render_widget(Paragraph::new("hi"), frame.area());
        });
        assert_eq!(buffer.area, Rect::new(0, 0, 7, 3));
        assert_eq!(buffer[(0, 0)].symbol(), "h");
        assert_eq!(buffer[(1, 0)].symbol(), "i");
    }

    #[test]
    fn text_has_one_line_per_row_and_keeps_trailing_spaces() {
        let buffer = render(5, 2, |frame| {
            frame.render_widget(Paragraph::new("ab"), frame.area());
        });
        assert_eq!(buffer_to_text(&buffer), "ab   \n     \n");
    }

    #[test]
    fn text_does_not_duplicate_wide_characters() {
        let buffer = render(4, 1, |frame| {
            frame.render_widget(Paragraph::new("日a"), frame.area());
        });
        assert_eq!(buffer_to_text(&buffer), "日a \n");
    }

    #[test]
    fn styled_lists_runs_of_identical_style() {
        let buffer = render(6, 2, |frame| {
            let line = ratatui::text::Line::from(vec!["ab".red().bold(), "cd".into()]);
            frame.render_widget(Paragraph::new(line), frame.area());
        });
        assert_eq!(
            buffer_to_styled(&buffer),
            "abcd  \n      \n\
             --- styles (fg/bg/modifiers) ---\n\
             0: 0..2 Red/Reset/BOLD | 2..6 Reset/Reset/NONE\n\
             1: 0..6 Reset/Reset/NONE\n"
        );
    }

    #[test]
    fn styled_keeps_wide_characters_in_one_run() {
        let buffer = render(7, 1, |frame| {
            let line = ratatui::text::Line::from("日本語x".on_blue());
            frame.render_widget(Paragraph::new(line), frame.area());
        });
        assert_eq!(
            buffer_to_styled(&buffer),
            "日本語x\n\
             --- styles (fg/bg/modifiers) ---\n\
             0: 0..7 Reset/Blue/NONE\n"
        );
    }
}
