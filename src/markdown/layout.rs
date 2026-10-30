//! Laying out a block tree as wrapped, prefixed lines.

use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::Styles;
use super::blocks::{Block, List};
use super::highlight::highlight_code;
use super::prose::{self, Inline};
use super::table;
use super::wrap::wrap_spans;

const BULLET: &str = "• ";
const QUOTE_BAR: &str = "│ ";
const CODE_INDENT: usize = 2;
const NEST_INDENT: usize = 2;

/// Lays out `blocks` at `width` cells with one blank line between blocks, or
/// none when `tight`.
pub(super) fn layout(
    blocks: &[Block],
    width: usize,
    styles: &Styles,
    tight: bool,
) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for block in blocks {
        let lines = block_lines(block, width, styles);
        if lines.is_empty() {
            continue;
        }
        if !out.is_empty() && !tight {
            out.push(Line::default());
        }
        out.extend(lines);
    }
    out
}

fn block_lines(block: &Block, width: usize, styles: &Styles) -> Vec<Line<'static>> {
    match block {
        Block::Paragraph { inline, .. } => paragraph_lines(inline, width),
        Block::Code { lang, text } => code_lines(lang.as_deref(), text, width, styles),
        Block::Rule => vec![Line::from(Span::styled("─".repeat(width), styles.muted))],
        Block::Quote(children) => quote_lines(children, width, styles),
        Block::List(list) => list_lines(list, width, styles),
        Block::Table(t) => table::lines(t, width, styles),
    }
}

fn paragraph_lines(inline: &Inline, width: usize) -> Vec<Line<'static>> {
    if inline.is_empty() {
        return Vec::new();
    }
    inline
        .split_lines()
        .iter()
        .flat_map(|line| prose::wrap(line, width))
        .map(Line::from)
        .collect()
}

fn code_lines(lang: Option<&str>, text: &str, width: usize, styles: &Styles) -> Vec<Line<'static>> {
    let inner = width.saturating_sub(CODE_INDENT).max(1);
    let indent = " ".repeat(CODE_INDENT);
    highlight_code(text, lang.unwrap_or(""), styles.theme)
        .into_iter()
        .flat_map(|line| wrap_spans(&line.spans, inner))
        .map(|spans| {
            if spans.is_empty() {
                Line::default()
            } else {
                let mut line = vec![Span::raw(indent.clone())];
                line.extend(spans);
                Line::from(line)
            }
        })
        .collect()
}

/// Puts `first` in front of the first line and `rest` in front of the others.
/// Blank lines get `rest` only when `pad_blank` is set.
fn prefix(
    lines: Vec<Line<'static>>,
    first: Span<'static>,
    rest: Span<'static>,
    pad_blank: bool,
) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let lead = if i == 0 { first.clone() } else { rest.clone() };
            if i > 0 && line.spans.is_empty() && !pad_blank {
                return line;
            }
            let mut spans = vec![lead];
            spans.extend(line.spans);
            Line::from(spans)
        })
        .collect()
}

fn quote_lines(children: &[Block], width: usize, styles: &Styles) -> Vec<Line<'static>> {
    let inner = width.saturating_sub(QUOTE_BAR.width()).max(1);
    let bar = Span::styled(QUOTE_BAR, styles.muted);
    prefix(
        layout(children, inner, styles, false),
        bar.clone(),
        bar,
        true,
    )
}

/// The marker of each item, padded to one width: `• ` or `1. `.
fn markers(list: &List) -> Vec<String> {
    let raw: Vec<String> = match list.start {
        None => list.items.iter().map(|_| BULLET.to_string()).collect(),
        Some(start) => (start..)
            .zip(&list.items)
            .map(|(n, _)| format!("{n}. "))
            .collect(),
    };
    let widest = raw.iter().map(|m| m.width()).max().unwrap_or(0);
    raw.into_iter().map(|m| format!("{m:<widest$}")).collect()
}

fn list_lines(list: &List, width: usize, styles: &Styles) -> Vec<Line<'static>> {
    let markers = markers(list);
    let indent = markers.first().map_or(0, |m| m.width());
    let mut out: Vec<Line<'static>> = Vec::new();
    for (item, marker) in list.items.iter().zip(markers) {
        if !out.is_empty() && !list.tight {
            out.push(Line::default());
        }
        let marker = Span::styled(marker, styles.base);
        let mut lines = item_lines(item, marker, indent, width, list, styles);
        if lines.is_empty() {
            lines.push(Line::default());
        }
        out.extend(lines);
    }
    out
}

/// One list item: the marker in front of its first line, the text hanging
/// under the first line's text, and nested lists indented by two cells.
fn item_lines(
    item: &[Block],
    marker: Span<'static>,
    indent: usize,
    width: usize,
    list: &List,
    styles: &Styles,
) -> Vec<Line<'static>> {
    let mut out: Vec<Line<'static>> = Vec::new();
    for (i, block) in item.iter().enumerate() {
        let nested = i > 0 && matches!(block, Block::List(_));
        let pad = if nested { NEST_INDENT } else { indent };
        let lines = block_lines(block, width.saturating_sub(pad).max(1), styles);
        if lines.is_empty() {
            continue;
        }
        let hang = Span::raw(" ".repeat(pad));
        let lead = if out.is_empty() {
            marker.clone()
        } else {
            hang.clone()
        };
        if !out.is_empty() && !list.tight {
            out.push(Line::default());
        }
        out.extend(prefix(lines, lead, hang, false));
    }
    out
}
