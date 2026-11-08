//! Follow mode and the `↓ N new` count (product §6.2).
//!
//! While the viewport shows the last row the transcript *follows*: new
//! content scrolls into view. Once the user scrolls or selects so that the
//! last row is off screen it stops, keeps the position, and counts the
//! sections that arrive. Reaching the end again (`G`, or scrolling down to
//! it) resumes following and resets the count.
//!
//! The initial load (and a reload after a `Reset`) is *loading*: the view
//! stays where it is until `Loaded` arrives, which decides whether to follow.

use std::collections::HashSet;
use std::sync::Arc;

use super::Transcript;
use crate::components::turn_block::SectionId;
use crate::model::Turn;

impl Transcript {
    /// Marks the transcript as loading: it does not follow until
    /// [`Loaded`](crate::model::ModelUpdate::Loaded) arrives.
    #[must_use]
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        if loading {
            self.following = false;
        }
        self
    }

    /// Whether new content scrolls into view.
    pub fn following(&self) -> bool {
        self.following
    }

    /// How many sections arrived since the user left the end.
    pub fn new_sections(&self) -> usize {
        self.new_sections
    }

    /// Whether the viewport shows the last row (also when everything fits).
    ///
    /// The prepared window answers it without laying out anything: the last
    /// block of the window must be the last block of the transcript, and end
    /// within the viewport. (A block is never empty, so a later block means
    /// more rows.)
    fn at_bottom(&mut self) -> bool {
        let Some(size) = self.viewport else {
            return false;
        };
        let end = self.offset + usize::from(size.height);
        let blocks = self.cache.len();
        if let Some(window) = self
            .window
            .as_ref()
            .filter(|w| w.size == size && w.offset == self.offset)
        {
            match window.blocks.last() {
                Some(last) => {
                    return last.index + 1 == blocks && last.top + last.layout.height() <= end;
                }
                None if blocks == 0 => return true,
                None => {}
            }
        }
        !self.measured(|cache, m| cache.covers(end + 1, m))
    }

    /// Starts or stops following. Stopping starts a new count; resuming
    /// resets it.
    fn set_following(&mut self, following: bool) {
        if following == self.following {
            return;
        }
        self.following = following;
        self.new_sections = 0;
        self.tail_from = self.content.turns.len().saturating_sub(1);
    }

    /// Decides whether the view follows from where it is: called after
    /// everything the user does that can move the view.
    pub(super) fn update_following(&mut self) {
        if self.loading || self.viewport.is_none() {
            return;
        }
        let at_bottom = self.at_bottom();
        self.set_following(at_bottom);
    }

    /// Scrolls so that the last row is the bottom row of the viewport.
    pub(super) fn pin_to_bottom(&mut self) {
        let Some(size) = self.viewport else {
            return;
        };
        let total = self.measured(|cache, m| cache.total(m));
        self.offset = total.saturating_sub(usize::from(size.height));
    }

    /// The initial load is complete: follow if the last row is visible.
    pub(super) fn finish_loading(&mut self) {
        self.loading = false;
        self.following = false;
        self.new_sections = 0;
        self.tail_from = self.content.turns.len().saturating_sub(1);
        self.update_following();
    }

    /// A reload from scratch starts: show the top again and wait for `Loaded`.
    pub(super) fn start_loading(&mut self) {
        self.loading = true;
        self.following = false;
        self.new_sections = 0;
        self.tail_from = 0;
    }

    /// The sections, before a change, of the turns of `batch` that count as
    /// new content when they grow: the last turn at the time the user left
    /// the end, and the ones after it.
    pub(super) fn tail_sections(&self, batch: &[Arc<Turn>]) -> Vec<(usize, HashSet<SectionId>)> {
        batch
            .iter()
            .filter_map(|turn| self.content.index.get(turn.id()).copied())
            .filter(|&at| at >= self.tail_from)
            .map(|at| (at, self.sections_of(at).into_iter().collect()))
            .collect()
    }

    /// Counts what a change added: the sections of the turns appended since
    /// `old_len` and the sections that appeared in `grown` turns.
    pub(super) fn count_new(&mut self, old_len: usize, grown: Vec<(usize, HashSet<SectionId>)>) {
        let appended: usize = (old_len..self.content.turns.len())
            .map(|at| self.sections_of(at).len())
            .sum();
        let added: usize = grown
            .into_iter()
            .filter(|&(at, _)| at < old_len)
            .map(|(at, before)| {
                self.sections_of(at)
                    .into_iter()
                    .filter(|id| !before.contains(id))
                    .count()
            })
            .sum();
        self.new_sections += appended + added;
    }
}
