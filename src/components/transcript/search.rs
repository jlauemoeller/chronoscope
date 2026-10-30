//! Searching the transcript (SPEC §6.2): the transcript's part of it.
//!
//! The search itself runs on a worker (see [`crate::transcript_search`]); the
//! `Transcript` owns what is shown of it. The exchange is made of attributes,
//! because `Component::attr` cannot answer a `Msg`:
//!
//! 1. The App sets [`SEARCH_QUERY_ATTR`] ([`SearchStart`]). The transcript
//!    now waits for the answer to that request.
//! 2. The App reads [`SEARCH_REQUEST_ATTR`] ([`SearchRequest`]): the query,
//!    a snapshot of the turns and the generation. It starts the search with
//!    them (`Msg::StartTranscriptSearch`).
//! 3. The answer is set with [`SEARCH_RESULTS_ATTR`] ([`SearchMatches`]).
//!    One for another generation than the one waited for is dropped. The
//!    transcript highlights the query and jumps to the first match at or
//!    after the selection (the first one if there is none).
//! 4. The App reads [`SEARCH_POSITION_ATTR`] (an `Option<SearchPosition>`)
//!    to show `3/17`. Keys that move among the matches report the position
//!    themselves as `Msg::SearchPosition`.
//!
//! # Temporary unfolding
//!
//! A jump into a folded section unfolds it, and, when the match is in
//! output past `truncate_output_lines`, expands the output. Both are
//! *temporary*: the transcript remembers them ([`TempUnfold`]) and undoes
//! them as soon as the selection is on another section (the `… N more
//! lines` row of the section counts as the section). A fold command on the
//! section makes the state explicit instead: `Enter` or `unfold` keeps the
//! section unfolded (it was the user who unfolded it now), `fold` folds it,
//! and the fold-all and unfold-all commands settle every section. (So the
//! first `Enter` on a section the search unfolded changes nothing on screen;
//! the second folds it.) Folding takes the search's expansion of the output
//! with it.

use std::collections::HashMap;
use std::sync::Arc;

use tuirealm::command::CmdResult;
use tuirealm::props::{AttrValue, Attribute};

use super::Transcript;
use super::folding::FoldOp;
use super::nav::{Reveal, Step, reveal};
use crate::components::payload_value;
use crate::components::turn_block::{SectionContent, SectionId, derive_sections};
use crate::model::ToolKind;
use crate::transcript_search::{
    Match, SearchCorpus, SearchMatches, SearchPosition, SearchQuery, SearchRequest, section_text,
};

/// Starts waiting for the answer to a search (a [`SearchStart`] payload).
pub const SEARCH_QUERY_ATTR: Attribute = Attribute::Custom("search_query");
/// The search the transcript waits for, as a [`SearchRequest`] payload (query
/// only); none when it waits for nothing.
pub const SEARCH_REQUEST_ATTR: Attribute = Attribute::Custom("search_request");
/// The answer to a search (a [`SearchMatches`] payload).
pub const SEARCH_RESULTS_ATTR: Attribute = Attribute::Custom("search_results");
/// Where the user is among the matches (an `Option<SearchPosition>` payload;
/// `None` without a search; query only).
pub const SEARCH_POSITION_ATTR: Attribute = Attribute::Custom("search_position");

/// `Cmd::Custom` name: go to the next match.
pub const SEARCH_NEXT: &str = "search_next";
/// `Cmd::Custom` name: go to the previous match.
pub const SEARCH_PREV: &str = "search_prev";
/// `Cmd::Custom` name: clear the search.
pub const CLEAR_SEARCH: &str = "clear_search";

/// A search the App asks the transcript to wait for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchStart {
    /// What to look for.
    pub query: String,
    /// The number of the request; unique over the run of the application.
    pub generation: u64,
}

/// The attribute value that starts waiting for `start`.
pub fn search_start_value(start: SearchStart) -> AttrValue {
    payload_value(start)
}

/// The attribute value that hands the transcript the answer to a search.
pub fn search_results_value(results: SearchMatches) -> AttrValue {
    payload_value(results)
}

/// What the search has unfolded and not yet folded again.
#[derive(Debug, Clone, PartialEq, Eq)]
struct TempUnfold {
    section: SectionId,
    /// The section was folded and the search unfolded it.
    unfolded: bool,
    /// The section's output was truncated and the search expanded it.
    expanded: bool,
}

/// A request that has not been answered.
#[derive(Debug, Clone)]
struct Awaiting {
    generation: u64,
    query: SearchQuery,
}

/// The matches of the search on screen.
#[derive(Debug, Clone)]
struct Active {
    matches: Arc<Vec<Match>>,
    /// The match the user is at.
    current: Option<usize>,
}

/// The search state of a transcript.
#[derive(Debug, Clone, Default)]
pub(super) struct SearchState {
    awaiting: Option<Awaiting>,
    active: Option<Active>,
    temp: Option<TempUnfold>,
}

impl SearchState {
    /// Whether there is a search, or one that is waited for.
    pub(super) fn is_on(&self) -> bool {
        self.awaiting.is_some() || self.active.is_some()
    }
}

impl Transcript {
    /// Starts waiting for the answer to `start`. An empty query is ignored.
    pub(super) fn begin_search(&mut self, start: SearchStart) {
        let Some(query) = SearchQuery::new(&start.query) else {
            return;
        };
        self.search.awaiting = Some(Awaiting {
            generation: start.generation,
            query,
        });
    }

    /// The request to run for the search that is waited for.
    pub(super) fn search_request(&self) -> Option<SearchRequest> {
        let awaiting = self.search.awaiting.as_ref()?;
        Some(SearchRequest {
            generation: awaiting.generation,
            query: awaiting.query.text().to_owned(),
            corpus: SearchCorpus::new(self.content.turns.clone(), self.content.show_system),
        })
    }

    /// Where the user is among the matches; `None` without a search.
    pub(super) fn search_position(&self) -> Option<SearchPosition> {
        let active = self.search.active.as_ref()?;
        Some(SearchPosition {
            current: active.current.map_or(0, |current| current + 1),
            total: active.matches.len(),
        })
    }

    /// Forgets the search without touching what it unfolded (the folds are
    /// gone too: a reload).
    pub(super) fn forget_search(&mut self) {
        self.search = SearchState::default();
        self.content.highlight = None;
    }

    /// Shows the matches of a search: highlights the query and jumps to the
    /// first match at or after the selection. Answers to another request
    /// than the one waited for are dropped.
    pub(super) fn apply_search_results(&mut self, results: &SearchMatches) {
        if self
            .search
            .awaiting
            .as_ref()
            .is_none_or(|awaiting| awaiting.generation != results.generation())
        {
            log::debug!("dropping the matches of search {}", results.generation());
            return;
        }
        let Some(awaiting) = self.search.awaiting.take() else {
            return;
        };
        // A new search replaces the previous one.
        self.undo_temp_unfold_keeping_screen();
        let matches = Arc::clone(results.matches());
        self.content.highlight = Some(awaiting.query);
        let empty = matches.is_empty();
        self.search.active = Some(Active {
            matches,
            current: None,
        });
        if !empty {
            let first = self.first_match_from_selection();
            self.goto_match(first, Step::Next);
        }
        self.refresh();
    }

    /// The match to start at: the first one in the selected section or after
    /// it; the first one if there is none.
    fn first_match_from_selection(&self) -> usize {
        let Some(active) = &self.search.active else {
            return 0;
        };
        let Some(selected) = &self.content.selected else {
            return 0;
        };
        let selected = selected.more_parent().unwrap_or_else(|| selected.clone());
        let Some(&turn) = self.content.index.get(&selected.turn) else {
            return 0;
        };
        let position = |block: usize, section: &SectionId| {
            derive_sections(&self.content.turns[block], true)
                .iter()
                .position(|s| s.id == *section)
        };
        let Some(selected_at) = position(turn, &selected) else {
            return 0;
        };
        let mut cache: HashMap<&SectionId, Option<usize>> = HashMap::new();
        for (index, found) in active.matches.iter().enumerate() {
            let Some(&block) = self.content.index.get(&found.section.turn) else {
                continue;
            };
            if block > turn {
                return index;
            }
            if block == turn {
                let at = *cache
                    .entry(&found.section)
                    .or_insert_with(|| position(block, &found.section));
                if at.is_some_and(|at| at >= selected_at) {
                    return index;
                }
            }
        }
        0
    }

    /// `n` / `N`: moves to the next or previous match, wrapping around.
    pub(super) fn step_search(&mut self, step: Step) -> CmdResult {
        let Some(active) = &self.search.active else {
            return CmdResult::NoChange;
        };
        let total = active.matches.len();
        if total == 0 {
            return CmdResult::NoChange;
        }
        let start = match (active.current, step) {
            (Some(current), Step::Next) => (current + 1) % total,
            (Some(current), Step::Previous) => (current + total - 1) % total,
            (None, _) => self.first_match_from_selection(),
        };
        if self.goto_match(start, step) {
            CmdResult::Visual
        } else {
            CmdResult::NoChange
        }
    }

    /// `Esc`: clears the search, the highlights and the temporary unfolds.
    pub(super) fn clear_search(&mut self) -> CmdResult {
        if !self.search.is_on() {
            return CmdResult::NoChange;
        }
        self.undo_temp_unfold_keeping_screen();
        self.forget_search();
        self.refresh();
        CmdResult::Visual
    }

    /// Goes to match `start`, or, if its section is not there, to the next
    /// match in the direction of `step` that is in a section that is.
    /// Returns whether it went anywhere.
    fn goto_match(&mut self, start: usize, step: Step) -> bool {
        let Some(active) = self.search.active.as_ref() else {
            return false;
        };
        let matches = Arc::clone(&active.matches);
        let total = matches.len();
        let advance = |index: usize| match step {
            Step::Next => (index + 1) % total,
            Step::Previous => (index + total - 1) % total,
        };
        let (mut index, mut visited) = (start, 0);
        while visited < total {
            if self.reveal_match(index) {
                if let Some(active) = self.search.active.as_mut() {
                    active.current = Some(index);
                }
                return true;
            }
            // Its section is not there: pass over all of its matches.
            let section = &matches[index].section;
            while visited < total && matches[index].section == *section {
                index = advance(index);
                visited += 1;
            }
        }
        false
    }

    /// Selects the section of match `index`, unfolding it (and expanding its
    /// output) if the match is hidden, and scrolls the match into view.
    /// Returns false, changing nothing, when the section is not there.
    fn reveal_match(&mut self, index: usize) -> bool {
        let Some(active) = &self.search.active else {
            return false;
        };
        let active_matches = Arc::clone(&active.matches);
        let Some(found) = active_matches.get(index).cloned() else {
            return false;
        };
        let section = found.section.clone();
        let Some(&block) = self.content.index.get(&section.turn) else {
            return false;
        };
        let listed = derive_sections(&self.content.turns[block], self.content.show_system)
            .iter()
            .any(|s| s.id == section);
        if !listed {
            return false;
        }
        let source = self.section_source(block, &section);
        let in_section = section_matches(&active_matches, index);
        let Some(query) = self.content.highlight.clone() else {
            return false;
        };
        let Some(mut view) = self.section_view(block, &section, &source) else {
            // The section draws no rows: there is nothing to select.
            return false;
        };
        if self
            .search
            .temp
            .as_ref()
            .is_some_and(|t| t.section != section)
        {
            self.undo_temp_unfold();
        }
        let mut temp = self
            .search
            .temp
            .take()
            .filter(|t| t.section == section)
            .unwrap_or(TempUnfold {
                section: section.clone(),
                unfolded: false,
                expanded: false,
            });
        if self.is_folded(&section) == Some(true) {
            self.content.folds.set_folded(section.clone(), false);
            self.cache.invalidate(block);
            temp.unfolded = true;
            view = self.section_view(block, &section, &source).unwrap_or(view);
        }
        let mut row = view.row_of(&found, &query, &in_section);
        if row.is_none() && view.truncated {
            // The line of the match is among those cut off.
            self.content.folds.set_expanded(section.clone());
            self.cache.invalidate(block);
            temp.expanded = true;
            if let Some(expanded) = self.section_view(block, &section, &source) {
                row = expanded.row_of(&found, &query, &in_section);
            }
        }
        self.search.temp = (temp.unfolded || temp.expanded).then_some(temp);
        self.content.selected = Some(section);
        // The selection and the heights changed: `cursor` and `span` need a
        // window that is up to date.
        self.refresh();
        self.scroll_to_match(block, row);
        self.refresh();
        true
    }

    /// Scrolls so that the selected section is in view, and, when it is
    /// taller than the viewport, the match's `row` of block `block`.
    fn scroll_to_match(&mut self, block: usize, row: Option<usize>) {
        let (Some(size), Some(cur)) = (self.viewport, self.cursor()) else {
            return;
        };
        let height = usize::from(size.height);
        let span = self.span(&cur, height);
        match row {
            Some(row) if span.len() > height => {
                let at = self.measured(|cache, m| cache.top(block, m)) + row;
                if !(self.offset..self.offset + height).contains(&at) {
                    self.offset = at.saturating_sub(height / 2);
                }
            }
            _ => self.offset = reveal(self.offset, height, &span, Reveal::Top),
        }
    }

    /// The plain text of `section`, as the search saw it.
    fn section_source(&self, block: usize, section: &SectionId) -> Source {
        let sections = derive_sections(&self.content.turns[block], self.content.show_system);
        let Some(found) = sections.iter().find(|s| s.id == *section) else {
            return Source::new(String::new(), false, false);
        };
        // Only markdown draws `- `, `> ` or `#` as something else than itself.
        let markdown = match found.content {
            SectionContent::Prompt(_)
            | SectionContent::Text(_)
            | SectionContent::Thinking(_)
            | SectionContent::Compaction(_) => true,
            SectionContent::Tool(call) => matches!(call.tool, ToolKind::Subagent),
            SectionContent::ModelChange(_)
            | SectionContent::Stats(_)
            | SectionContent::System(_) => false,
        };
        // A diff draws the context lines of an edit once.
        let diff = matches!(
            found.content,
            SectionContent::Tool(call) if matches!(call.tool, ToolKind::Edit | ToolKind::Write)
        );
        Source::new(section_text(found).into_owned(), markdown, diff)
    }

    /// The rows `section` draws now; `None` when it draws none.
    fn section_view<'a>(
        &self,
        block: usize,
        section: &SectionId,
        source: &'a Source,
    ) -> Option<SectionView<'a>> {
        let layout = self.content.block(block).layout(self.nav_width());
        let rows = layout
            .section_rows()
            .iter()
            .find(|(id, _)| id == section)?
            .1
            .clone();
        let truncated = layout
            .section_rows()
            .iter()
            .any(|(id, _)| id.more_parent().as_ref() == Some(section));
        let rows: Vec<(usize, String)> = rows
            .filter_map(|row| Some((row, layout.row_text(row)?)))
            .collect();
        Some(SectionView::new(rows, truncated, source))
    }

    /// Undoes what the search unfolded, if the selection has moved to another
    /// section. Called after every command.
    pub(super) fn settle_temp_unfold(&mut self) {
        let Some(temp) = &self.search.temp else {
            return;
        };
        let here = self
            .content
            .selected
            .as_ref()
            .map(|id| id.more_parent().unwrap_or_else(|| id.clone()));
        if here.as_ref() != Some(&temp.section) {
            self.undo_temp_unfold_keeping_screen();
        }
    }

    /// Folds and collapses what the search opened, keeping the selected
    /// section at the screen row it is at.
    fn undo_temp_unfold_keeping_screen(&mut self) {
        if self.search.temp.is_none() {
            return;
        }
        let anchor = self.anchor_selected();
        self.undo_temp_unfold();
        self.restore_anchor(anchor);
        self.refresh();
    }

    fn undo_temp_unfold(&mut self) {
        let Some(temp) = self.search.temp.take() else {
            return;
        };
        let Some(&block) = self.content.index.get(&temp.section.turn) else {
            return;
        };
        let before = self.sections_if(|_| true);
        if temp.expanded {
            self.content.folds.clear_expanded(&temp.section);
        }
        if temp.unfolded {
            self.content.folds.set_folded(temp.section, true);
        }
        self.cache.invalidate(block);
        self.restore_selection(before);
    }

    /// A fold command is about to act on `section`: it makes the temporary
    /// state explicit. Returns whether the command is already done: `Enter`
    /// or `unfold` on a section the search unfolded keeps it unfolded (the
    /// key looks inert: the second `Enter` is the toggle).
    pub(super) fn fold_command_on_temp(&mut self, section: &SectionId, op: FoldOp) -> bool {
        let Some(temp) = self.search.temp.take_if(|t| t.section == *section) else {
            return false;
        };
        if temp.unfolded && op != FoldOp::Fold {
            // Unfolded by the user from now on; an expansion the search made
            // stays with it.
            return true;
        }
        if op == FoldOp::Unfold {
            // Already unfolded by the user, and only expanded by the search:
            // nothing to do, and the expansion is still temporary.
            self.search.temp = Some(temp);
            return false;
        }
        // The command folds the section: the expansion goes with it.
        self.collapse_expansion(&temp);
        false
    }

    fn collapse_expansion(&mut self, temp: &TempUnfold) {
        if temp.expanded {
            self.content.folds.clear_expanded(&temp.section);
            if let Some(&block) = self.content.index.get(&temp.section.turn) {
                self.cache.invalidate(block);
            }
        }
    }

    /// The user expanded the output of `section` (`Enter` on its `… N more
    /// lines` row): that expansion is theirs.
    pub(super) fn output_expanded_by_user(&mut self, section: &SectionId) {
        if let Some(temp) = self.search.temp.as_mut().filter(|t| t.section == *section) {
            temp.expanded = false;
            if !temp.unfolded {
                self.search.temp = None;
            }
        }
    }

    /// The user folded or unfolded every section: nothing is temporary.
    /// Folding everything takes the search's expansion with it.
    pub(super) fn forget_temp_unfold(&mut self, folding: bool) {
        if let Some(temp) = self.search.temp.take()
            && folding
        {
            self.collapse_expansion(&temp);
        }
    }
}

/// The `(line, start)` of the matches in the section of match `index`, in
/// order (they are next to each other in `matches`).
fn section_matches(matches: &[Match], index: usize) -> Vec<(usize, usize)> {
    let section = &matches[index].section;
    let first = matches[..index]
        .iter()
        .rposition(|m| m.section != *section)
        .map_or(0, |at| at + 1);
    matches[first..]
        .iter()
        .take_while(|m| m.section == *section)
        .map(|m| (m.line, m.range.start))
        .collect()
}

/// The characters of `text` that are not white space, and not the `→` of a
/// numbered line (the Read renderer draws a gutter instead): what stays the
/// same when a line is wrapped, indented or numbered.
fn squash(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_whitespace() && *c != '→')
        .collect()
}

/// Whether `prefix` (squashed) holds only what the renderers draw in front
/// of a line: digits (line numbers, list numbers), `.`, the diff marks `+`
/// and `-`, the list bullet `•`, the quote bar `│` and the fold markers.
fn is_gutter(prefix: &str) -> bool {
    prefix
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | '•' | '│' | '▶' | '▼'))
}

/// How many lines before a match are looked at for one that is drawn, to
/// anchor the search for a line that is drawn differently from its source.
const ANCHOR_LOOKBACK: usize = 200;

/// The lines of a section's plain text, split and squashed once.
struct Source {
    text: String,
    bounds: Vec<std::ops::Range<usize>>,
    /// Where each line's content starts, after the markdown markers (`- `,
    /// `1. `, `> `, `## `) that are drawn as a gutter or not at all.
    bodies: Vec<usize>,
    squashed: Vec<String>,
    /// Whether the section draws the context lines of a diff once.
    diff: bool,
}

/// The fence of a fenced code block that `line` opens or closes: its
/// character and length. A fence is at least three `` ` `` or `~`, indented
/// by at most three spaces.
fn fence_of(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let fence = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = rest.chars().take_while(|&c| c == fence).count();
    (len >= 3).then_some((fence, len))
}

/// The bytes at the start of `line` that are markdown markers: block quote
/// bars, list bullets and numbers, heading hashes.
fn marker_len(line: &str) -> usize {
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start();
        let after = if let Some(after) = trimmed.strip_prefix('>') {
            after
        } else if let Some(after) = ["- ", "* ", "+ "]
            .iter()
            .find_map(|bullet| trimmed.strip_prefix(bullet))
        {
            after
        } else if let Some(after) = trimmed
            .trim_start_matches('#')
            .strip_prefix(' ')
            .filter(|_| trimmed.starts_with('#'))
        {
            after
        } else {
            let digits = trimmed.len()
                - trimmed
                    .trim_start_matches(|c: char| c.is_ascii_digit())
                    .len();
            match trimmed[digits..].strip_prefix(['.', ')']) {
                Some(after) if digits > 0 && after.starts_with(' ') => after,
                _ => break,
            }
        };
        rest = after;
    }
    line.len() - rest.len()
}

impl Source {
    /// The lines of `text`. With `markdown`, the markers that markdown draws
    /// differently are left out of the content of the lines, except in fenced
    /// code blocks, which draw them as they are.
    fn new(text: String, markdown: bool, diff: bool) -> Self {
        let mut bounds = Vec::new();
        let mut bodies = Vec::new();
        let mut fence: Option<(char, usize)> = None;
        let mut start = 0;
        for line in text.split('\n') {
            bounds.push(start..start + line.len());
            start += line.len() + 1;
            let body = if !markdown {
                0
            } else if let Some((open, len)) = fence {
                // It closes on a fence of the same character, at least as
                // long, with nothing after it.
                if fence_of(line).is_some_and(|(c, n)| c == open && n >= len)
                    && line.trim().chars().all(|c| c == open)
                {
                    fence = None;
                }
                0
            } else if let Some(opened) = fence_of(line) {
                fence = Some(opened);
                0
            } else {
                marker_len(line)
            };
            bodies.push(body);
        }
        let squashed = bounds
            .iter()
            .zip(&bodies)
            .map(|(b, body)| squash(&text[b.start + body..b.end]))
            .collect();
        Self {
            text,
            bounds,
            bodies,
            squashed,
            diff,
        }
    }

    fn line(&self, index: usize) -> Option<&str> {
        Some(&self.text[self.bounds.get(index)?.clone()])
    }

    /// The content of line `index` before byte `at` of the line, without
    /// the markers; empty when `at` is within them.
    fn content_before(&self, index: usize, at: usize) -> String {
        let (Some(line), Some(&body)) = (self.line(index), self.bodies.get(index)) else {
            return String::new();
        };
        squash(line.get(body..at.max(body)).unwrap_or_default())
    }

    /// Whether line `other` is a drawing of `line` for the purposes of
    /// counting: it is `line` after nothing but a gutter.
    fn is_drawing_of(&self, other: usize, line: &str) -> bool {
        let other = &self.squashed[other];
        other.strip_suffix(line).is_some_and(is_gutter)
    }
}

/// What a section draws, to find the row a match is on.
///
/// A match knows the line of the section's plain text it is in. That line is
/// looked for among the drawn rows, ignoring white space: it is one row (or a
/// few, when wrapped) that ends where the line ends and starts at the row's
/// start or after a gutter ([`is_gutter`]). That shows labels such as
/// `Result:` for what they are. A line that occurs more than once is found as
/// the n-th drawing, counting the earlier source lines by the same rule.
///
/// A line that is not found is either cut off by the truncation (the
/// caller expands the output and asks again) or drawn differently from its
/// source (inline markdown, a string of a JSON input). Then the query is
/// looked for in the drawn rows after the nearest earlier line that is
/// found. The matches between that line and this one say which occurrence it
/// is. Only a match whose text is not drawn at all gets no row.
struct SectionView<'a> {
    /// The drawn rows with their text, counted from the block's top border.
    rows: Vec<(usize, String)>,
    /// Whether the output is cut with a `… N more lines` row.
    truncated: bool,
    source: &'a Source,
    /// The squashed rows joined, and where each starts in it.
    joined: String,
    starts: Vec<usize>,
}

impl<'a> SectionView<'a> {
    fn new(rows: Vec<(usize, String)>, truncated: bool, source: &'a Source) -> Self {
        let mut joined = String::new();
        let mut starts = Vec::with_capacity(rows.len());
        for (_, text) in &rows {
            starts.push(joined.len());
            joined.push_str(&squash(text));
        }
        Self {
            rows,
            truncated,
            source,
            joined,
            starts,
        }
    }

    /// The drawn row that holds `found`; `None` when it is not drawn, or when
    /// its line was cut off by the truncation.
    ///
    /// `in_section` is the `(line, start)` of every match of the section.
    fn row_of(
        &self,
        found: &Match,
        query: &SearchQuery,
        in_section: &[(usize, usize)],
    ) -> Option<usize> {
        if let Some(start) = self.place(found.line) {
            let at = start
                + self
                    .source
                    .content_before(found.line, found.range.start)
                    .len();
            return Some(self.row_at(at));
        }
        if self.truncated {
            return None;
        }
        self.anchored(found, query, in_section)
    }

    /// The index in `rows` of the row that holds the byte `at` of the joined
    /// rows.
    fn index_at(&self, at: usize) -> usize {
        self.starts.partition_point(|&s| s <= at).saturating_sub(1)
    }

    fn row_at(&self, at: usize) -> usize {
        self.rows[self.index_at(at)].0
    }

    /// Where in the joined rows source line `index` is drawn.
    ///
    /// The drawings are counted against the source lines that read the same
    /// (`written`, this one included):
    /// - more drawings than lines: one belongs to no such line (the wrapped
    ///   end of a longer line, say), so none can be told from the others;
    /// - as many: the n-th drawing is the n-th line;
    /// - fewer: lines were cut off by the truncation (the n-th drawing, if it
    ///   is there; the caller expands and asks again), or the section draws
    ///   a line less often than it is written.
    fn place(&self, index: usize) -> Option<usize> {
        let line = self.source.squashed.get(index)?;
        if line.is_empty() {
            return None;
        }
        let (mut earlier, mut written) = (0, 0);
        for (i, _) in self.source.squashed.iter().enumerate() {
            if self.source.is_drawing_of(i, line) {
                written += 1;
                if i < index {
                    earlier += 1;
                }
            }
        }
        let (drawn, nth, last) = self.drawings(line, earlier);
        if drawn > written {
            None
        } else if drawn == written || self.truncated {
            nth
        } else if self.source.diff {
            // The context lines of a diff are written in the old and in the
            // new text and drawn once. When such a line repeats in the file,
            // the surplus occurrences land on the row of the last copy.
            last
        } else {
            None
        }
    }

    /// How many times the squashed `line` is drawn, where the `nth` drawing
    /// starts and where the last one does.
    fn drawings(&self, line: &str, nth: usize) -> (usize, Option<usize>, Option<usize>) {
        let mut seen = 0;
        let mut found_nth = None;
        let mut last = None;
        let mut from = 0;
        while let Some(found) = self.joined[from..].find(line) {
            let at = from + found;
            if self.is_drawing(at, line.len()) {
                if seen == nth {
                    found_nth = Some(at);
                }
                seen += 1;
                last = Some(at);
            }
            from = at + self.joined[at..].chars().next().map_or(1, char::len_utf8);
        }
        (seen, found_nth, last)
    }

    /// Whether `at..at + len` of the joined rows is a drawn line: it ends
    /// with a row and starts with one or after a gutter.
    fn is_drawing(&self, at: usize, len: usize) -> bool {
        let end = at + len;
        let Some(last) = self.starts.partition_point(|&s| s < end).checked_sub(1) else {
            return false;
        };
        if self.starts[last] + squash(&self.rows[last].1).len() != end {
            return false;
        }
        let first = self.index_at(at);
        is_gutter(&self.joined[self.starts[first]..at])
    }

    /// The row of the occurrence of the query that `found` is, counted from
    /// the nearest earlier line that is drawn as written, and not looking
    /// past the next later line that is.
    ///
    /// The rows between the two make the window. Matches in it whose text is
    /// not drawn (a link URL, a fence info string) are counted but have no
    /// occurrence, so the window may hold fewer than the match needs: then
    /// the last one stands in for it, and a window without any starts at
    /// its first row.
    fn anchored(
        &self,
        found: &Match,
        query: &SearchQuery,
        in_section: &[(usize, usize)],
    ) -> Option<usize> {
        let from = found.line.saturating_sub(ANCHOR_LOOKBACK);
        let anchor = (from..found.line).rev().find_map(|line| {
            let start = self.place(line)?;
            let end = start + self.source.squashed[line].len();
            Some((line, self.index_at(end.saturating_sub(1))))
        });
        let first = anchor.map_or(0, |(_, row)| row + 1);
        if first >= self.rows.len() {
            return None;
        }
        // Source order is not draw order: a later line drawn above the
        // window does not end it.
        let until = (found.line + 1
            ..self
                .source
                .squashed
                .len()
                .min(found.line + 1 + ANCHOR_LOOKBACK))
            .find_map(|line| {
                let row = self.index_at(self.place(line)?);
                (row >= first).then_some(row)
            })
            .unwrap_or(self.rows.len());
        let before = in_section
            .iter()
            .filter(|&&(line, start)| {
                anchor.is_none_or(|(anchor, _)| line > anchor)
                    && (line, start) < (found.line, found.range.start)
            })
            .count();
        let window = &self.rows[first..until.max(first + 1)];
        let mut seen = 0;
        let mut last = None;
        for (row, text) in window {
            if is_label(text) {
                continue;
            }
            let count = query.find_all(text).len();
            if count > 0 {
                last = Some(*row);
            }
            seen += count;
            if seen > before {
                return Some(*row);
            }
        }
        Some(last.unwrap_or(window[0].0))
    }
}

/// Whether a drawn row is a label such as `Result:` or `Command:`.
fn is_label(row: &str) -> bool {
    let row = row.trim();
    row.len() <= 12
        && row.strip_suffix(':').is_some_and(|name| {
            name.chars().next().is_some_and(char::is_uppercase)
                && name.chars().all(|c| c.is_alphabetic() || c == ' ')
        })
}
