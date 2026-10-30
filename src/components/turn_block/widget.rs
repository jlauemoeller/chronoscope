//! The `TurnBlock` widget: one turn as a rounded block of sections.

use std::ops::Range;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Widget;

use super::emphasis::emphasize;
use super::renderers::{DEFAULT_OPEN_KEY, Rendered, SectionCtx, render_section, text_width};
use super::section::{FoldState, Section, SectionId, derive_sections};
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
    truncate: usize,
    open_key: &'a str,
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
            truncate: crate::config::DEFAULT_TRUNCATE_OUTPUT_LINES,
            open_key: DEFAULT_OPEN_KEY,
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

    /// Sets `truncate_output_lines`: tool output longer than this many lines
    /// is cut, with a `… N more lines` row that is a selection stop.
    #[must_use]
    pub fn truncate_output_lines(mut self, lines: usize) -> Self {
        self.truncate = lines;
        self
    }

    /// Sets the label of the key that opens a subagent transcript, for the
    /// hint of subagent sections.
    #[must_use]
    pub fn open_subagent_key(mut self, label: &'a str) -> Self {
        self.open_key = label;
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
        for section in derive_sections(self.turn, self.show_system) {
            let ctx = self.section_ctx(&section, text_width, folds);
            let Rendered { lines, more } = render_section(&section, &ctx);
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
            let more_id = section.id.more_row();
            spans.push((section.id, start..rows.len() + 1));
            // The truncation row follows its section directly and is a
            // selection stop of its own.
            if let (Some(more), Some(id)) = (more, more_id) {
                self.push_more(more, id, &mut rows, &mut spans);
            }
        }
        if !spans.is_empty() {
            rows.push(blank());
        }
        BlockLayout {
            rows,
            sections: spans,
        }
    }

    fn section_ctx<'s>(
        &'s self,
        section: &Section<'_>,
        width: u16,
        folds: &FoldState,
    ) -> SectionCtx<'s> {
        SectionCtx {
            theme: self.theme,
            width,
            folded: folds.is_folded(section),
            truncate: self.truncate,
            expanded: folds.is_expanded(&section.id),
            open_key: self.open_key,
        }
    }

    /// Adds the `… N more lines` row `id` below the section just pushed.
    fn push_more(
        &self,
        line: Line<'static>,
        id: SectionId,
        rows: &mut Vec<BlockRow>,
        spans: &mut Vec<(SectionId, Range<usize>)>,
    ) {
        let selected = self.selected == Some(&id);
        let line = emphasize(vec![line], selected, self.dim_unselected, self.theme).remove(0);
        let start = rows.len() + 1;
        rows.push(BlockRow { line, selected });
        spans.push((id, start..rows.len() + 1));
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
        let (symbol, style) = if row.selected {
            ("▒", self.theme.style(Token::SelectionMarker))
        } else {
            ("│", self.border_style())
        };
        buf.set_string(area.x, y, symbol, style);
        buf.set_string(
            area.x + 1,
            y,
            " ".repeat(usize::from(area.width.saturating_sub(2))),
            Style::default(),
        );
        // The content may use columns 2 to W - 4; the rest is padding.
        buf.set_line(area.x + 2, y, &row.line, area.width.saturating_sub(6));
        buf.set_string(area.x + area.width - 1, y, "│", self.border_style());
    }

    /// Draws a layout made by [`layout`](Self::layout) for `area.width`,
    /// without laying the block out again. A caller that cached the layout
    /// (the virtualised transcript) renders it every frame this way.
    pub fn render_layout(&self, layout: &BlockLayout, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.width < 2 || area.height == 0 {
            return;
        }
        for dy in 0..area.height {
            let row = self.skip + usize::from(dy);
            if row >= layout.height() {
                break;
            }
            self.draw_row(layout, row, area, area.y + dy, buf);
        }
    }
}

impl Widget for &TurnBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        if area.width < 2 || area.height == 0 {
            return;
        }
        self.render_layout(&self.layout(area.width), area, buf);
    }
}

impl Widget for TurnBlock<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        (&self).render(area, buf);
    }
}
