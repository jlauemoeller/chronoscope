//! The `TurnBlock` widget: one turn as a rounded block of sections.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use super::emphasis::emphasize;
use super::section::{FoldState, SectionId, sections};
use super::sections::{render_section, text_width};
use super::title::TitleBar;
use crate::model::Turn;
use crate::theme::{Role, Theme, Token};
use crate::timestamp::TimestampFormatter;

/// A row of the block between the top and the bottom border.
#[derive(Debug, Clone)]
struct BlockRow {
    /// The content from column 2 on.
    line: Line<'static>,
    /// Whether the row belongs to the selected section.
    selected: bool,
}

/// The rows of a block laid out for one width.
#[derive(Debug, Clone)]
pub struct BlockLayout {
    rows: Vec<BlockRow>,
    sections: Vec<(SectionId, Range<usize>)>,
}

impl BlockLayout {
    /// The number of rows of the whole block, borders included.
    pub fn height(&self) -> usize {
        self.rows.len() + 2
    }

    /// The rows each section covers, counted from the top border (row 0).
    pub fn section_rows(&self) -> &[(SectionId, Range<usize>)] {
        &self.sections
    }
}

/// One turn drawn as a rounded block (SPEC §5.2).
///
/// The title shows the role (and model and effort) on the left and the
/// timestamp on the right. The sections follow, separated by blank rows. The
/// selected section is marked by `▒` over the left border on its rows.
///
/// Layout is pure: [`height`](Self::height) reports exactly the number of rows
/// [`render`](Widget::render) draws when given the same width. To draw only
/// part of a block (the virtualised transcript), use
/// [`skip_rows`](Self::skip_rows) and give `render` an area of the height to
/// show; the rows drawn are those of the full block from `skip_rows` on.
#[derive(Debug, Clone)]
pub struct TurnBlock<'a> {
    turn: &'a Turn,
    theme: &'a Theme,
    formatter: &'a TimestampFormatter,
    folds: Option<&'a FoldState>,
    selected: Option<&'a SectionId>,
    show_system: bool,
    dim_unselected: bool,
    skip: usize,
}

impl<'a> TurnBlock<'a> {
    /// A block for `turn`, with every section at its default fold, nothing
    /// selected, system sections shown and unselected sections dimmed (the
    /// defaults of the configuration).
    pub fn new(turn: &'a Turn, theme: &'a Theme, formatter: &'a TimestampFormatter) -> Self {
        Self {
            turn,
            theme,
            formatter,
            folds: None,
            selected: None,
            show_system: true,
            dim_unselected: true,
            skip: 0,
        }
    }

    /// Sets the fold state.
    #[must_use]
    pub fn folds(mut self, folds: &'a FoldState) -> Self {
        self.folds = Some(folds);
        self
    }

    /// Sets the selected section.
    #[must_use]
    pub fn selected(mut self, selected: Option<&'a SectionId>) -> Self {
        self.selected = selected;
        self
    }

    /// Sets whether system / injected sections are shown.
    #[must_use]
    pub fn show_system(mut self, show: bool) -> Self {
        self.show_system = show;
        self
    }

    /// Sets whether unselected sections are drawn in `muted_foreground`.
    #[must_use]
    pub fn dim_unselected(mut self, dim: bool) -> Self {
        self.dim_unselected = dim;
        self
    }

    /// Skips the first `rows` rows of the block when rendering.
    #[must_use]
    pub fn skip_rows(mut self, rows: usize) -> Self {
        self.skip = rows;
        self
    }

    /// Lays the block out for `width` cells.
    pub fn layout(&self, width: u16) -> BlockLayout {
        let text_width = text_width(width);
        let default_folds = FoldState::new();
        let folds = self.folds.unwrap_or(&default_folds);
        let blank = || BlockRow {
            line: Line::default(),
            selected: false,
        };
        let mut rows = vec![blank()];
        let mut spans = Vec::new();
        for section in sections(self.turn, self.show_system) {
            let lines = render_section(&section, self.theme, text_width, folds.is_folded(&section));
            if lines.is_empty() {
                continue;
            }
            if !spans.is_empty() {
                rows.push(blank());
            }
            let selected = self.selected == Some(&section.id);
            let lines = emphasize(lines, selected, self.dim_unselected, self.theme);
            // Row 0 of the block is the top border.
            let start = rows.len() + 1;
            rows.extend(lines.into_iter().map(|line| BlockRow { line, selected }));
            spans.push((section.id, start..rows.len() + 1));
        }
        if !spans.is_empty() {
            rows.push(blank());
        }
        BlockLayout {
            rows,
            sections: spans,
        }
    }

    /// The number of rows the block takes at `width`.
    pub fn height(&self, width: u16) -> usize {
        self.layout(width).height()
    }

    fn border_style(&self) -> Style {
        Style::new().fg(self.theme.role(Role::Border))
    }

    fn draw_row(&self, layout: &BlockLayout, row: usize, area: Rect, y: u16, buf: &mut Buffer) {
        let width = usize::from(area.width);
        let border = self.border_style();
        if row == 0 {
            let title = TitleBar::new(self.turn, self.theme, self.formatter);
            buf.set_line(area.x, y, &title.line(area.width), area.width);
        } else if row + 1 == layout.height() {
            let line = format!("╰{}╯", "─".repeat(width.saturating_sub(2)));
            buf.set_string(area.x, y, line, border);
        } else {
            self.draw_interior(&layout.rows[row - 1], area, y, buf);
        }
    }

    fn draw_interior(&self, row: &BlockRow, area: Rect, y: u16, buf: &mut Buffer) {
        let inner = area.width.saturating_sub(2);
        let mut spans = vec![Span::raw(" ")];
        spans.extend(row.line.spans.iter().cloned());
        let pad = usize::from(inner).saturating_sub(Line::from(spans.clone()).width());
        spans.push(Span::raw(" ".repeat(pad)));
        let (symbol, style) = if row.selected {
            ("▒", self.theme.style(Token::SelectionMarker))
        } else {
            ("│", self.border_style())
        };
        buf.set_string(area.x, y, symbol, style);
        buf.set_line(area.x + 1, y, &Line::from(spans), inner);
        buf.set_string(area.x + area.width - 1, y, "│", self.border_style());
    }
}

impl Widget for TurnBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.width < 2 || area.height == 0 {
            return;
        }
        let layout = self.layout(area.width);
        for dy in 0..area.height {
            let row = self.skip + usize::from(dy);
            if row >= layout.height() {
                break;
            }
            self.draw_row(&layout, row, area, area.y + dy, buf);
        }
    }
}
