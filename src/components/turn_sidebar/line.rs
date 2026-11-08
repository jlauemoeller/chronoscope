//! One row of the turns sidebar as a styled line (product §5.2, §5.3).
//!
//! Columns count from the pane's left border (column 0): the marker is in
//! column 3, labels in column 7 (the Compaction rule from column 4), and the
//! right margin ends two cells before the right border, `pane_width - 4`.
//! The line returned starts in column 1, inside the border.

use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::rows::{Entry, EntryKind, Row};
use crate::duration::format_duration;
use crate::theme::{Role, Theme, Token};

#[cfg(test)]
thread_local! {
    /// How many row lines were built on this thread (tests assert that only
    /// the rows a pane shows are).
    static LINES_BUILT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many row lines were built on this thread.
#[cfg(test)]
pub(crate) fn lines_built() -> usize {
    LINES_BUILT.with(std::cell::Cell::get)
}

/// The column of the marker.
const MARKER_COLUMN: usize = 3;
/// The column of the labels.
const LABEL_COLUMN: usize = 7;
/// The cells after the right margin: two blank cells and the border.
const RIGHT_CHROME: usize = 4;
/// The least width at which rows are drawn: a marker, a gap and a label cell.
pub const MIN_ROW_WIDTH: usize = LABEL_COLUMN + RIGHT_CHROME + 1;

/// The label of the Compaction row.
const COMPACTION: &str = "Compaction";

/// How the row is shown (product §5.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Look {
    /// Not selected.
    Plain,
    /// Selected while the sidebar has focus: an inverted band.
    SelectedFocused,
    /// Selected while the sidebar has no focus: bold `secondary_foreground`.
    SelectedUnfocused,
}

/// The styles of the marker and of the rest of the row.
struct Styles {
    marker: Style,
    body: Style,
}

impl Styles {
    fn new(theme: &Theme, look: Look) -> Self {
        let muted = Style::new().fg(theme.role(Role::MutedForeground));
        match look {
            Look::Plain => Self {
                marker: muted,
                body: muted,
            },
            Look::SelectedFocused => Self {
                marker: theme.style(Token::SelectionMarker),
                body: Style::new()
                    .fg(theme.role(Role::Foreground))
                    .add_modifier(Modifier::REVERSED),
            },
            Look::SelectedUnfocused => {
                let secondary = Style::new().fg(theme.role(Role::SecondaryForeground));
                Self {
                    marker: secondary,
                    body: secondary.add_modifier(Modifier::BOLD),
                }
            }
        }
    }
}

/// The line for `row` in a pane `pane_width` columns wide, or `None` when
/// the pane is too narrow to draw rows. A blank row is an empty line.
pub fn row_line(theme: &Theme, row: Row, look: Look, pane_width: usize) -> Option<Line<'static>> {
    if pane_width < MIN_ROW_WIDTH {
        return None;
    }
    #[cfg(test)]
    LINES_BUILT.with(|n| n.set(n.get() + 1));
    // The cells from column 4 to the right margin.
    let body_width = pane_width - RIGHT_CHROME - MARKER_COLUMN;
    let styles = Styles::new(theme, look);
    let (glyph, body) = match row {
        Row::Blank => return Some(Line::default()),
        Row::Connector => ("│", String::new()),
        Row::Start => ("┬", labelled("Start", None, body_width)),
        Row::End => ("┴", labelled("End", None, body_width)),
        Row::Entry(entry) => match entry.kind {
            EntryKind::User => ("├", labelled("User", None, body_width)),
            EntryKind::Assistant => ("├", assistant_body(entry, body_width)),
            EntryKind::Compaction => ("├", compaction_body(body_width)),
        },
    };
    let marker = if look == Look::Plain { glyph } else { "▒" };
    Some(Line::from(vec![
        Span::raw(" ".repeat(MARKER_COLUMN - 1)),
        Span::styled(marker, styles.marker),
        Span::styled(body, styles.body),
    ]))
}

fn assistant_body(entry: Entry, body_width: usize) -> String {
    let duration = entry.duration.map(format_duration);
    labelled("Assistant", duration.as_deref(), body_width)
}

/// The text from column 4 to the right margin of an entry with `label` in
/// column 7 and `duration` ending at the margin. The duration is left out
/// when it does not fit after the label with a cell between them.
fn labelled(label: &str, duration: Option<&str>, body_width: usize) -> String {
    let indent = LABEL_COLUMN - MARKER_COLUMN - 1;
    let room = body_width.saturating_sub(indent);
    let label: String = fit(label, room);
    let mut text = format!("{}{label}", " ".repeat(indent));
    if let Some(duration) = duration {
        let used = text.width();
        let duration_width = duration.width();
        if used + 1 + duration_width <= body_width {
            text.push_str(&" ".repeat(body_width - used - duration_width));
            text.push_str(duration);
        }
    }
    // The text runs to the right margin, so a band covers the whole row.
    let used = text.width();
    text.push_str(&" ".repeat(body_width.saturating_sub(used)));
    text
}

/// `╶╶ Compaction ╶╶╶…`, after the marker the caller supplies in column 3: the
/// rule runs from column 4, the label is in column 7 and the rule goes on to
/// the right margin.
fn compaction_body(body_width: usize) -> String {
    let lead = LABEL_COLUMN - MARKER_COLUMN - 1;
    let mut text = format!("{} ", "╶".repeat(lead - 1));
    text.push_str(COMPACTION);
    text.push(' ');
    let used = text.width();
    if used < body_width {
        text.push_str(&"╶".repeat(body_width - used));
    }
    fit(&text, body_width)
}

/// `text` cut to at most `width` cells.
fn fit(text: &str, width: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = unicode_width::UnicodeWidthChar::width(c).unwrap_or(0);
        if used + w > width {
            break;
        }
        used += w;
        out.push(c);
    }
    out
}
