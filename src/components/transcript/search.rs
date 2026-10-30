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
//! and the fold-all and unfold-all commands settle every section.

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
    Match, SearchCorpus, SearchMatches, SearchPosition, SearchQuery, SearchRequest,
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

/// The most matches a jump tries before it gives up: matches in sections
/// that are not there any more are passed over.
const MAX_SKIPPED: usize = 1000;

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
    /// The index of the first match of each section.
    first_of: HashMap<SectionId, usize>,
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
        let mut first_of = HashMap::new();
        for (index, found) in matches.iter().enumerate() {
            first_of.entry(found.section.clone()).or_insert(index);
        }
        self.content.highlight = Some(awaiting.query);
        let empty = matches.is_empty();
        self.search.active = Some(Active {
            matches,
            first_of,
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
    /// one in the direction of `step`. Returns whether it went anywhere.
    fn goto_match(&mut self, start: usize, step: Step) -> bool {
        let Some(total) = self.search.active.as_ref().map(|a| a.matches.len()) else {
            return false;
        };
        for skipped in 0..total.min(MAX_SKIPPED) {
            let index = match step {
                Step::Next => (start + skipped) % total,
                Step::Previous => (start + total - skipped % total) % total,
            };
            if self.reveal_match(index) {
                if let Some(active) = self.search.active.as_mut() {
                    active.current = Some(index);
                }
                return true;
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
        let Some(found) = active.matches.get(index) else {
            return false;
        };
        let section = found.section.clone();
        let nth = index - active.first_of.get(&section).copied().unwrap_or(index);
        let Some(&block) = self.content.index.get(&section.turn) else {
            return false;
        };
        let listed = derive_sections(&self.content.turns[block], self.content.show_system)
            .iter()
            .any(|s| s.id == section);
        if !listed || self.occurrences(block, &section).is_none() {
            return false;
        }
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
        }
        let mut seen = self.occurrences(block, &section);
        if seen
            .as_ref()
            .is_some_and(|seen| seen.truncated && seen.total() <= nth)
        {
            self.content.folds.set_expanded(section.clone());
            self.cache.invalidate(block);
            temp.expanded = true;
            seen = self.occurrences(block, &section);
        }
        self.search.temp = (temp.unfolded || temp.expanded).then_some(temp);
        let row = seen.and_then(|seen| seen.row_of(nth));
        self.content.selected = Some(section);
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

    /// Where the query occurs in the rows of `section` as they are drawn
    /// now; `None` when the section draws no rows.
    fn occurrences(&self, block: usize, section: &SectionId) -> Option<Occurrences> {
        let query = self.content.highlight.as_ref()?;
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
        let rows = rows
            .filter_map(|row| {
                let count = query.find_all(&layout.row_text(row)?).len();
                (count > 0).then_some((row, count))
            })
            .collect();
        Some(Occurrences { rows, truncated })
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
    /// or `unfold` on a section the search unfolded keeps it unfolded.
    pub(super) fn fold_command_on_temp(&mut self, section: &SectionId, op: FoldOp) -> bool {
        let Some(temp) = self.search.temp.take_if(|t| t.section == *section) else {
            return false;
        };
        if temp.unfolded && op != FoldOp::Fold {
            // Unfolded by the user from now on; the output stays expanded
            // only if the search expanded it, which the user keeps too.
            return true;
        }
        false
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
    pub(super) fn forget_temp_unfold(&mut self) {
        self.search.temp = None;
    }
}

/// The rows of a section that hold an occurrence of the query.
struct Occurrences {
    /// `(row counted from the block's top border, occurrences in it)`.
    rows: Vec<(usize, usize)>,
    /// Whether the output is cut with a `… N more lines` row.
    truncated: bool,
}

impl Occurrences {
    fn total(&self) -> usize {
        self.rows.iter().map(|&(_, count)| count).sum()
    }

    /// The row of the `nth` occurrence (from 0).
    fn row_of(&self, nth: usize) -> Option<usize> {
        let mut before = 0;
        for &(row, count) in &self.rows {
            if nth < before + count {
                return Some(row);
            }
            before += count;
        }
        None
    }
}
