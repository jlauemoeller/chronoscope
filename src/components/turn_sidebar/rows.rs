//! What the turns sidebar lists (SPEC §6.2, §6.6): one entry per turn that
//! has a block, and the rows drawn from them.
//!
//! [`TurnRows`] keeps one *slot* per turn, in transcript order, whether or
//! not the turn has an entry, plus a map from the turn's id to its slot. An
//! update therefore touches the slots of the turns it names and nothing else,
//! and a turn that gains or loses its entry keeps its place. The list drawn
//! is `Start`, the entries, `End`, with exactly one row between neighbours:
//! `│`, or a blank row when either neighbour is a Compaction. It follows that
//! the item `k` is on row `2k`, so a row is found without a row vector.
//!
//! Nothing here lays out or renders a block, and no tool output is read:
//! an entry is decided by [`derive_sections`], which only lists the sections.

use std::collections::HashMap;
use std::time::Duration;

use crate::components::turn_block::derive_sections;
use crate::model::{ModelUpdate, Turn, TurnId};

#[cfg(test)]
thread_local! {
    /// How many entries were derived from a turn on this thread (tests assert
    /// that an update rebuilds only the turns it names).
    static ENTRY_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// How many times an entry was derived from a turn on this thread.
#[cfg(test)]
pub(crate) fn entry_builds() -> usize {
    ENTRY_BUILDS.with(std::cell::Cell::get)
}

/// The kind of turn an entry stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A User turn.
    User,
    /// An Assistant turn.
    Assistant,
    /// A Compaction turn.
    Compaction,
}

/// One selectable row of the sidebar: a turn with a block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    /// What kind of turn it is.
    pub kind: EntryKind,
    /// The turn's duration, for an Assistant turn with stats.
    pub duration: Option<Duration>,
}

/// The entry of `turn`: `None` when it has no section that can form a block.
/// System content never forms a block of its own, so it is left out
/// (`show_system` false) and toggling it cannot change the entries.
pub fn entry_for(turn: &Turn) -> Option<Entry> {
    #[cfg(test)]
    ENTRY_BUILDS.with(|n| n.set(n.get() + 1));
    if derive_sections(turn, false).is_empty() {
        return None;
    }
    Some(match turn {
        Turn::User(_) => Entry {
            kind: EntryKind::User,
            duration: None,
        },
        Turn::Assistant(assistant) => Entry {
            kind: EntryKind::Assistant,
            duration: assistant.stats.as_ref().map(|stats| stats.duration),
        },
        Turn::Compaction(_) => Entry {
            kind: EntryKind::Compaction,
            duration: None,
        },
    })
}

/// One row of the list as drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Row {
    /// `● Start`, the first row. Not selectable.
    Start,
    /// `● End`, the last row. Not selectable.
    End,
    /// A turn.
    Entry(Entry),
    /// `│` between two `●` rows.
    Connector,
    /// A blank row next to a Compaction.
    Blank,
}

impl Row {
    /// Whether the row can be selected.
    pub fn selectable(self) -> bool {
        matches!(self, Row::Entry(_))
    }

    /// Whether the row is a Compaction, which breaks the timeline.
    fn is_compaction(self) -> bool {
        matches!(
            self,
            Row::Entry(Entry {
                kind: EntryKind::Compaction,
                ..
            })
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Slot {
    id: TurnId,
    entry: Option<Entry>,
}

/// The sidebar's data: a slot per turn and the entries among them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnRows {
    /// One slot per turn, in transcript order.
    slots: Vec<Slot>,
    /// The slot of every turn.
    index: HashMap<TurnId, usize>,
    /// The slots that have an entry, ascending: the entry `e` is
    /// `slots[entries[e]]`.
    entries: Vec<usize>,
}

impl TurnRows {
    /// The rows of `turns`, in order. One pass, deriving each turn's sections
    /// only; for the rows of a transcript that is opened or closed.
    pub fn from_turns<'a>(turns: impl IntoIterator<Item = &'a Turn>) -> Self {
        let mut rows = Self::default();
        for turn in turns {
            rows.set(turn);
        }
        rows
    }

    /// Applies what a model update says about the turns: `Appended` adds
    /// turns at the end, and a turn whose id is already held updates that
    /// turn's slot instead (as the transcript does); `Updated` replaces the
    /// slots of the turns it names and ignores unknown ids; `Reset` clears.
    /// Other updates change nothing. Only the turns named are looked at.
    pub fn apply(&mut self, update: &ModelUpdate) {
        match update {
            ModelUpdate::Appended(turns) => {
                for turn in turns {
                    self.set(turn);
                }
            }
            ModelUpdate::Updated(turns) => {
                for turn in turns {
                    if self.index.contains_key(turn.id()) {
                        self.set(turn);
                    }
                }
            }
            ModelUpdate::Reset => {
                *self = Self::default();
            }
            ModelUpdate::Progress(_)
            | ModelUpdate::Loaded
            | ModelUpdate::ParseWarnings(_)
            | ModelUpdate::Activity { .. } => {}
        }
    }

    /// Puts `turn` in its slot, appending a slot for a turn not held.
    fn set(&mut self, turn: &Turn) {
        let entry = entry_for(turn);
        match self.index.get(turn.id()) {
            Some(&at) => {
                let had = self.slots[at].entry.is_some();
                self.slots[at].entry = entry;
                match (had, entry.is_some()) {
                    (false, true) => {
                        let position = self.entries.partition_point(|&slot| slot < at);
                        self.entries.insert(position, at);
                    }
                    (true, false) => {
                        let position = self.entries.partition_point(|&slot| slot < at);
                        self.entries.remove(position);
                    }
                    _ => {}
                }
            }
            None => {
                let at = self.slots.len();
                self.index.insert(turn.id().clone(), at);
                self.slots.push(Slot {
                    id: turn.id().clone(),
                    entry,
                });
                if entry.is_some() {
                    self.entries.push(at);
                }
            }
        }
    }

    /// The number of turns held, with or without an entry.
    pub fn turn_count(&self) -> usize {
        self.slots.len()
    }

    /// The number of entries.
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// The entry `index` (counted among the entries only), with its turn.
    pub fn entry(&self, index: usize) -> Option<(&TurnId, Entry)> {
        let slot = self.slots.get(*self.entries.get(index)?)?;
        Some((&slot.id, slot.entry?))
    }

    /// The position among the entries of the turn `id`, when it has one.
    pub fn entry_index(&self, id: &TurnId) -> Option<usize> {
        let slot = *self.index.get(id)?;
        let position = self.entries.partition_point(|&s| s < slot);
        (self.entries.get(position) == Some(&slot)).then_some(position)
    }

    /// The number of rows of the list: `Start`, the entries and `End`, and
    /// one row between neighbours.
    pub fn row_count(&self) -> usize {
        2 * (self.entries.len() + 2) - 1
    }

    /// The row of the entry `index`.
    pub fn row_of_entry(index: usize) -> usize {
        2 * (index + 1)
    }

    /// The entry on row `row`, when the row is an entry's.
    pub fn entry_at_row(&self, row: usize) -> Option<usize> {
        (row.is_multiple_of(2) && row >= 2 && row < self.row_count() - 2).then(|| row / 2 - 1)
    }

    /// The row at position `row` of the list.
    pub fn row(&self, row: usize) -> Option<Row> {
        if row >= self.row_count() {
            return None;
        }
        let item = |k: usize| -> Row {
            if k == 0 {
                Row::Start
            } else if k == self.entries.len() + 1 {
                Row::End
            } else {
                self.entry(k - 1).map_or(Row::Blank, |(_, e)| Row::Entry(e))
            }
        };
        if row.is_multiple_of(2) {
            return Some(item(row / 2));
        }
        let (above, below) = (item(row / 2), item(row / 2 + 1));
        Some(if above.is_compaction() || below.is_compaction() {
            Row::Blank
        } else {
            Row::Connector
        })
    }
}
