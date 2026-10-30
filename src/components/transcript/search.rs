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
use crate::components::turn_block::{SectionId, derive_sections};
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
        let Some(found) = active.matches.get(index).cloned() else {
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
        let Some(mut view) = self.section_view(block, &section) else {
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
            view = self.section_view(block, &section).unwrap_or(view);
        }
        let mut row = view.row_of(&found);
        if row.is_none() && view.truncated {
            // The line of the match is among those cut off.
            self.content.folds.set_expanded(section.clone());
            self.cache.invalidate(block);
            temp.expanded = true;
            if let Some(expanded) = self.section_view(block, &section) {
                row = expanded.row_of(&found);
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

    /// The rows `section` draws now; `None` when it draws none.
    fn section_view(&self, block: usize, section: &SectionId) -> Option<SectionView> {
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
            .filter_map(|row| Some((row, squash(&layout.row_text(row)?))))
            .collect();
        let source = derive_sections(&self.content.turns[block], self.content.show_system)
            .into_iter()
            .find(|s| s.id == *section)
            .map(|s| section_text(&s).into_owned())
            .unwrap_or_default();
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

/// The characters of `text` that are not white space: what stays the same
/// when a line is wrapped or indented.
fn squash(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// How far into a row, in squashed bytes, a source line may start: the
/// gutter of a numbered or diff row in front of it.
const GUTTER: usize = 12;

/// What a section draws, to find the row a match is on.
///
/// A match knows the line of the section's plain text it is in. That line is
/// looked for among the drawn rows, ignoring white space: it is one row (or a
/// few, when wrapped) that ends where the line ends and starts at the row's
/// start or after a short gutter. That shows labels such as `Result:` for
/// what they are, and a line that is not drawn (cut off by the truncation,
/// or changed by markdown) does not shift the matches that are.
struct SectionView {
    /// The drawn rows, counted from the block's top border, squashed.
    rows: Vec<(usize, String)>,
    /// Whether the output is cut with a `… N more lines` row.
    truncated: bool,
    /// The plain text of the section.
    source: String,
    /// The squashed rows joined, and where each starts in it.
    joined: String,
    starts: Vec<usize>,
}

impl SectionView {
    fn new(rows: Vec<(usize, String)>, truncated: bool, source: String) -> Self {
        let mut joined = String::new();
        let mut starts = Vec::with_capacity(rows.len());
        for (_, text) in &rows {
            starts.push(joined.len());
            joined.push_str(text);
        }
        Self {
            rows,
            truncated,
            source,
            joined,
            starts,
        }
    }

    /// The drawn row that holds the start of `found`; `None` when its line is
    /// not drawn.
    ///
    /// A line that is not drawn as it is written (markdown changes `- ` to a
    /// bullet, say) is looked for by its end, from the match on, unless the
    /// output is cut: then a line that is not found was cut, and some other
    /// row ending alike must not stand for it.
    fn row_of(&self, found: &Match) -> Option<usize> {
        let lines: Vec<&str> = self.source.split('\n').collect();
        let text = lines.get(found.line)?;
        let line = squash(text);
        if line.is_empty() {
            return None;
        }
        // The same line may occur more than once: this is the n-th.
        let earlier = lines[..found.line]
            .iter()
            .filter(|other| squash(other) == line)
            .count();
        let at = match self.nth_drawn(&line, earlier) {
            Some(start) => start + squash(&text[..found.range.start]).len(),
            None if self.truncated => return None,
            None => {
                let tail = squash(&text[found.range.start..]);
                let earlier = lines[..found.line]
                    .iter()
                    .filter(|other| squash(other).ends_with(&tail))
                    .count();
                self.nth_ending(&tail, earlier)?
            }
        };
        let row = self.starts.partition_point(|&s| s <= at).checked_sub(1)?;
        Some(self.rows[row].0)
    }

    /// Where the `nth` drawing of the squashed `tail` starts, which ends
    /// where a row ends (and may start in an earlier row).
    fn nth_ending(&self, tail: &str, nth: usize) -> Option<usize> {
        let mut seen = 0;
        let mut from = 0;
        while let Some(found) = self.joined[from..].find(tail) {
            let at = from + found;
            if self.ends_a_row(at + tail.len()) {
                if seen == nth {
                    return Some(at);
                }
                seen += 1;
            }
            from = at + self.joined[at..].chars().next().map_or(1, char::len_utf8);
        }
        None
    }

    /// Whether the joined rows have a row end at `end`.
    fn ends_a_row(&self, end: usize) -> bool {
        self.starts
            .partition_point(|&s| s < end)
            .checked_sub(1)
            .is_some_and(|last| self.starts[last] + self.rows[last].1.len() == end)
    }

    /// Where in the joined rows the `nth` drawing of the squashed `line`
    /// starts.
    fn nth_drawn(&self, line: &str, nth: usize) -> Option<usize> {
        let mut seen = 0;
        let mut from = 0;
        while let Some(found) = self.joined[from..].find(line) {
            let at = from + found;
            if self.is_drawing(at, line.len()) {
                if seen == nth {
                    return Some(at);
                }
                seen += 1;
            }
            from = at + self.joined[at..].chars().next().map_or(1, char::len_utf8);
        }
        None
    }

    /// Whether `at..at + len` of the joined rows is a drawn line: it ends
    /// with a row and starts with one or within its gutter.
    fn is_drawing(&self, at: usize, len: usize) -> bool {
        if !self.ends_a_row(at + len) {
            return false;
        }
        let Some(first) = self.starts.partition_point(|&s| s <= at).checked_sub(1) else {
            return false;
        };
        at - self.starts[first] <= GUTTER
    }
}
