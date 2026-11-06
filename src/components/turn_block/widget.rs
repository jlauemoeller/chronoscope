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
use crate::transcript_search::SearchQuery;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// A row of the block between the top and the bottom border.
#[derive(Debug, Clone)]
struct BlockRow {
    /// The content from column 2 on.
    line: Line<'static>,
    /// Whether the row belongs to the selected section.
    selected: bool,
    /// The cell of `line` (counted from column 2) where the rule and the hint
    /// of a subagent's title row start. Such a row is drawn up to column
    /// `W - 3`, two cells into the padding, and what is drawn from here on is
    /// chrome, not text.
    chrome: Option<usize>,
}

impl BlockRow {
    /// The text of the row: the line without the chrome.
    fn text(&self) -> String {
        let Some(from) = self.chrome else {
            return self.line.to_string();
        };
        let mut width = 0;
        let mut text = String::new();
        for c in self.line.spans.iter().flat_map(|s| s.content.chars()) {
            if width >= from {
                break;
            }
            width += c.width().unwrap_or(0);
            text.push(c);
        }
        text
    }

    /// How many cells of the line are drawn, from column 2, in a block of
    /// `width` columns: the text area and the lead, or two cells more for a
    /// title row.
    fn drawn_cells(&self, width: u16) -> u16 {
        let reach = if self.chrome.is_some() { 4 } else { 6 };
        width.saturating_sub(reach)
    }
}

/// The rows of a block laid out for one width.
#[derive(Debug, Clone)]
pub struct BlockLayout {
    rows: Vec<BlockRow>,
    sections: Vec<(SectionId, Range<usize>)>,
    width: u16,
}

#[cfg(test)]
thread_local! {
    /// How many blocks were laid out or rendered on this thread.
    static BLOCK_WORK: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many blocks were laid out or rendered on this thread: the work the
/// turns sidebar must not cause (SPEC §6.6).
#[cfg(test)]
pub(crate) fn block_work() -> usize {
    BLOCK_WORK.with(std::cell::Cell::get)
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

    /// The text of row `row` (counted from the top border, like
    /// [`section_rows`](Self::section_rows)), from column 2 on, without the
    /// rule and the hint of a subagent's title row. `None` for the borders
    /// and rows that do not exist.
    pub fn row_text(&self, row: usize) -> Option<String> {
        let interior = row.checked_sub(1)?;
        self.rows.get(interior).map(BlockRow::text)
    }

    /// The first column after the part of row `row` (counted like
    /// [`row_text`](Self::row_text)) that belongs to its section: the text
    /// area, which ends at column `W - 5`, or on a title row that draws its
    /// rule or hint into the padding, the cells it draws there. Column
    /// `W - 2` is never part of a row.
    pub fn content_end(&self, row: usize) -> u16 {
        let text_end = self.width.saturating_sub(4);
        let Some(row) = row.checked_sub(1).and_then(|i| self.rows.get(i)) else {
            return text_end;
        };
        if row.chrome.is_none() {
            return text_end;
        }
        let drawn = u16::try_from(row.line.width()).unwrap_or(u16::MAX);
        let limit = self.width.saturating_sub(2);
        text_end.max(drawn.saturating_add(2).min(limit))
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
    search: Option<&'a SearchQuery>,
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
            search: None,
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

    /// Highlights every occurrence of `query` in the rows it draws with the
    /// `search_match` style. This is part of drawing only: it changes no
    /// layout, so a layout made without it can be drawn with it.
    #[must_use]
    pub fn search(mut self, query: Option<&'a SearchQuery>) -> Self {
        self.search = query;
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
        #[cfg(test)]
        BLOCK_WORK.with(|n| n.set(n.get() + 1));
        let text_width = text_width(width);
        let default_folds = FoldState::new();
        let folds = self.folds.unwrap_or(&default_folds);
        let blank = || BlockRow {
            line: Line::default(),
            selected: false,
            chrome: None,
        };
        let mut rows = vec![blank()];
        let mut spans = Vec::new();
        for section in derive_sections(self.turn, self.show_system) {
            let ctx = self.section_ctx(&section, text_width, folds);
            let rendered = render_section(&section, &ctx);
            if rendered.lines.is_empty() {
                continue;
            }
            if !spans.is_empty() {
                rows.push(blank());
            }
            self.push_section(section.id, rendered, &mut rows, &mut spans);
        }
        if !spans.is_empty() {
            rows.push(blank());
        }
        BlockLayout {
            rows,
            sections: spans,
            width,
        }
    }

    /// Adds the rows of the section `id`, emphasised, and the `… N more
    /// lines` row below it, if it has one.
    fn push_section(
        &self,
        id: SectionId,
        rendered: Rendered,
        rows: &mut Vec<BlockRow>,
        spans: &mut Vec<(SectionId, Range<usize>)>,
    ) {
        let Rendered {
            lines,
            more,
            chrome,
        } = rendered;
        let selected = self.selected == Some(&id);
        // The rule keeps its colour: it is not emphasised.
        let keep = chrome.and_then(|c| Some((c.row, c.rule?)));
        let lines = emphasize(lines, selected, self.dim_unselected, self.theme, keep);
        // Row 0 of the block is the top border.
        let start = rows.len() + 1;
        rows.extend(lines.into_iter().enumerate().map(|(i, line)| BlockRow {
            line,
            selected,
            chrome: chrome.filter(|c| c.row == i).map(|c| c.from),
        }));
        let more_id = id.more_row();
        spans.push((id, start..rows.len() + 1));
        // The truncation row follows its section directly and is a selection
        // stop of its own.
        if let (Some(more), Some(id)) = (more, more_id) {
            self.push_more(more, id, rows, spans);
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
            verbatim: false,
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
        let line = emphasize(vec![line], selected, self.dim_unselected, self.theme, None).remove(0);
        let start = rows.len() + 1;
        rows.push(BlockRow {
            line,
            selected,
            chrome: None,
        });
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
        // The content may use columns 2 to W - 5; the rest is padding, but
        // for the two cells a title row reaches into it.
        buf.set_line(area.x + 2, y, &row.line, row.drawn_cells(area.width));
        self.highlight(row, area, y, buf);
        buf.set_string(area.x + area.width - 1, y, "│", self.border_style());
    }

    /// Draws the occurrences of the search query in the content of `row`
    /// with the `search_match` style. A match is looked for in each row on
    /// its own, so one that wraps onto the next row is not highlighted.
    fn highlight(&self, row: &BlockRow, area: Rect, y: u16, buf: &mut Buffer) {
        let Some(query) = self.search else {
            return;
        };
        let text = row.text();
        let style = self.theme.style(Token::SearchMatch);
        let first = area.x + 2;
        let columns = usize::from(row.drawn_cells(area.width));
        for found in query.find_all(&text) {
            let start = text[..found.start].width();
            let end = start + text[found.clone()].width();
            for column in start..end.min(columns) {
                // `column` is below `columns`, which fits in a `u16`.
                buf[(first + column as u16, y)].set_style(style);
            }
        }
    }

    /// Draws a layout made by [`layout`](Self::layout) for `area.width`,
    /// without laying the block out again. A caller that cached the layout
    /// (the virtualised transcript) renders it every frame this way.
    pub fn render_layout(&self, layout: &BlockLayout, area: Rect, buf: &mut Buffer) {
        #[cfg(test)]
        BLOCK_WORK.with(|n| n.set(n.get() + 1));
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
