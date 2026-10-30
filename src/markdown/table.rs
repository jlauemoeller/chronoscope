//! Laying out a table.

use pulldown_cmark::Alignment;
use ratatui::text::{Line, Span};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::Styles;
use super::blocks::Table;
use super::prose::{Inline, Seg};

const SEPARATOR: &str = " │ ";
const RULE_CROSS: &str = "─┼─";
const ELLIPSIS: &str = "…";

/// Lays out `table` in at most `width` cells where it can.
///
/// Columns are as wide as their widest cell. When the table is too wide the
/// widest columns are narrowed one cell at a time, and the cells that no longer
/// fit end in `…`. Columns never get narrower than one cell.
pub(super) fn lines(table: &Table, width: usize, styles: &Styles) -> Vec<Line<'static>> {
    let columns = table
        .rows
        .iter()
        .chain(std::iter::once(&table.header))
        .map(Vec::len)
        .max()
        .unwrap_or(0);
    if columns == 0 {
        return Vec::new();
    }
    let widths = column_widths(table, columns, width);
    let mut out = vec![row_line(&table.header, &widths, &table.aligns, styles)];
    out.push(rule_line(&widths, styles));
    out.extend(
        table
            .rows
            .iter()
            .map(|row| row_line(row, &widths, &table.aligns, styles)),
    );
    out
}

/// The width of each column: its natural width, narrowed to fit `width`.
fn column_widths(table: &Table, columns: usize, width: usize) -> Vec<usize> {
    let mut widths = vec![1; columns];
    for row in table.rows.iter().chain(std::iter::once(&table.header)) {
        for (col, cell) in row.iter().enumerate() {
            widths[col] = widths[col].max(cell.width());
        }
    }
    let separators = SEPARATOR.width() * (columns - 1);
    while widths.iter().sum::<usize>() + separators > width {
        let Some((widest, &w)) = widths.iter().enumerate().rev().max_by_key(|(_, w)| **w) else {
            break;
        };
        if w <= 1 {
            break;
        }
        widths[widest] -= 1;
    }
    widths
}

fn rule_line(widths: &[usize], styles: &Styles) -> Line<'static> {
    let cells: Vec<String> = widths.iter().map(|w| "─".repeat(*w)).collect();
    Line::from(Span::styled(cells.join(RULE_CROSS), styles.muted))
}

fn row_line(
    row: &[Inline],
    widths: &[usize],
    aligns: &[Alignment],
    styles: &Styles,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for (col, width) in widths.iter().enumerate() {
        if col > 0 {
            spans.push(Span::styled(SEPARATOR, styles.muted));
        }
        let align = aligns.get(col).copied().unwrap_or(Alignment::None);
        let last = col + 1 == widths.len();
        spans.extend(cell_spans(row.get(col), *width, align, last));
    }
    Line::from(spans)
}

/// A cell fitted to `width`: truncated with `…` when too wide, and padded
/// according to `align`. The last column of left-aligned cells is not padded.
fn cell_spans(
    cell: Option<&Inline>,
    width: usize,
    align: Alignment,
    last: bool,
) -> Vec<Span<'static>> {
    let (mut spans, used) = match cell {
        Some(cell) => truncate(&cell.0, width),
        None => (Vec::new(), 0),
    };
    let pad = width - used;
    let (left, right) = match align {
        Alignment::Right => (pad, 0),
        Alignment::Center => (pad / 2, pad - pad / 2),
        Alignment::Left | Alignment::None => (0, if last { 0 } else { pad }),
    };
    if left > 0 {
        spans.insert(0, Span::raw(" ".repeat(left)));
    }
    if right > 0 {
        spans.push(Span::raw(" ".repeat(right)));
    }
    spans
}

/// `segs` cut to at most `max` cells, ending in `…` when anything was cut.
/// Returns the spans and their width.
fn truncate(segs: &[Seg], max: usize) -> (Vec<Span<'static>>, usize) {
    let total: usize = segs.iter().map(|seg| seg.text.width()).sum();
    if total <= max {
        let spans = segs
            .iter()
            .map(|seg| Span::styled(seg.text.clone(), seg.style))
            .collect();
        return (spans, total);
    }
    let budget = max.saturating_sub(1);
    let mut spans: Vec<Span<'static>> = Vec::new();
    let mut used = 0;
    'cut: for seg in segs {
        let mut kept = String::new();
        for g in seg.text.graphemes(true) {
            let w = g.width();
            if used + w > budget {
                push_kept(&mut spans, &mut kept, seg);
                break 'cut;
            }
            kept.push_str(g);
            used += w;
        }
        push_kept(&mut spans, &mut kept, seg);
    }
    let style = segs
        .iter()
        .find(|seg| !seg.text.is_empty())
        .map(|seg| seg.style)
        .unwrap_or_default();
    let style = spans.last().map_or(style, |span| span.style);
    spans.push(Span::styled(ELLIPSIS, style));
    (spans, used + ELLIPSIS.width())
}

fn push_kept(spans: &mut Vec<Span<'static>>, kept: &mut String, seg: &Seg) {
    if !kept.is_empty() {
        spans.push(Span::styled(std::mem::take(kept), seg.style));
    }
}
