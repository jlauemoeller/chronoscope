//! References from a turn's sections to the source they were read from, and
//! what a harness reads there for the details overlay (SPEC cycle 2 §5.2).
//!
//! Turns keep only cheap references (a byte range for a Claude line, a row id
//! for OpenCode), never the field values:
//! most of what the overlay shows varies across versions and is rarely looked
//! at, so it is read again from the source when the overlay opens.

use std::io;
use std::ops::Range;
use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::{ByteSource, ItemId};

/// A reference to a line of a transcript: the bytes it lives in and its byte
/// range (without the newline).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRef {
    /// The transcript.
    pub bytes: ByteSource,
    /// Byte range of the line.
    pub range: Range<u64>,
}

impl SourceRef {
    /// Reads the line again.
    pub fn read(&self) -> io::Result<Vec<u8>> {
        self.bytes.read(self.range.clone())
    }
}

/// Which part of a turn a reference is for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKey {
    /// The prompt of a User turn, or the boundary of a Compaction turn.
    Own,
    /// The stats footer of an Assistant turn.
    Footer,
    /// An item of the turn.
    Item(ItemId),
}

/// The lines of one message of an Assistant turn. A message is split over
/// several lines (one per content block), and `usage` and `stop_reason` are
/// complete only on the later ones.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MessageLines {
    /// The message's latest line.
    latest: Range<u64>,
    /// Its latest line with a non-null `stop_reason`.
    stopped: Option<Range<u64>>,
}

/// Where an item's reference points.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ItemSource {
    /// A content block of the message with this number in the turn.
    Message(usize),
    /// One line.
    Line(Range<u64>),
    /// A tool call: the line of its call, and the line of its result once
    /// that has arrived.
    Call {
        call: Range<u64>,
        result: Option<Range<u64>>,
    },
    /// A row of a database (OpenCode): its id, and whether the item is a
    /// tool call, which is a part of the row.
    Row { row: Arc<str>, call: bool },
}

/// The references of one turn, by section. A side table, so that the items
/// carry nothing of it. Claude's entries are only added, so the position of
/// an item's entry never changes; OpenCode's are removed when the item goes
/// (see [`remove_item`](Self::remove_item)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TurnSources {
    /// The transcript all the ranges are in: one for the whole turn.
    bytes: Option<ByteSource>,
    /// The line of a prompt, or of a compaction's boundary.
    own: Option<Range<u64>>,
    /// The last `turn_duration` line.
    footer: Option<Range<u64>>,
    messages: Vec<MessageLines>,
    items: Vec<(ItemId, ItemSource)>,
    /// The row of a prompt (OpenCode).
    own_row: Option<Arc<str>>,
    /// The first assistant row of an Assistant turn (OpenCode): what the
    /// turn's own fields are read from.
    turn_row: Option<Arc<str>>,
}

impl TurnSources {
    /// No references.
    pub fn new() -> Self {
        Self::default()
    }

    fn bytes_of(&mut self, bytes: &ByteSource) {
        if self.bytes.is_none() {
            self.bytes = Some(bytes.clone());
        }
    }

    /// Sets the line of the prompt or the compaction.
    pub fn set_own(&mut self, bytes: &ByteSource, range: &Range<u64>) {
        self.bytes_of(bytes);
        replace(&mut self.own, range);
    }

    /// Sets the line of the footer: the last `turn_duration` line. Returns
    /// whether the table changed.
    pub fn set_footer(&mut self, bytes: &ByteSource, range: &Range<u64>) -> bool {
        self.bytes_of(bytes);
        replace(&mut self.footer, range)
    }

    /// Adds a message, first seen on the line `range`, and returns its
    /// number in the turn.
    pub fn add_message(&mut self, bytes: &ByteSource, range: &Range<u64>) -> usize {
        self.bytes_of(bytes);
        self.messages.push(MessageLines {
            latest: range.clone(),
            stopped: None,
        });
        self.messages.len() - 1
    }

    /// Notes a line of message `message`; `stopped` is whether it records a
    /// `stop_reason`. Returns whether the table changed.
    pub fn note_message_line(&mut self, message: usize, range: &Range<u64>, stopped: bool) -> bool {
        let Some(lines) = self.messages.get_mut(message) else {
            return false;
        };
        let mut changed = replace_range(&mut lines.latest, range);
        if stopped {
            changed |= replace(&mut lines.stopped, range);
        }
        changed
    }

    /// Adds item `id`, which the caller knows is not in the table yet, as a
    /// content block of message `message`. Nothing here looks the item up
    /// (a turn can hold tens of thousands of items), so adding is constant
    /// time. An item added again is found by its last entry.
    pub fn add_message_item(&mut self, id: &ItemId, message: usize) {
        self.items.push((id.clone(), ItemSource::Message(message)));
    }

    /// Adds item `id`, which the caller knows is not in the table yet, with
    /// one line.
    pub fn add_line(&mut self, bytes: &ByteSource, id: &ItemId, range: &Range<u64>) {
        self.bytes_of(bytes);
        self.items
            .push((id.clone(), ItemSource::Line(range.clone())));
    }

    /// Adds the tool call `id`, which the caller knows is not in the table
    /// yet, with the line of its call. Returns the call's number in the
    /// table, to give to [`set_result`](Self::set_result).
    pub fn add_call(&mut self, bytes: &ByteSource, id: &ItemId, range: &Range<u64>) -> usize {
        self.bytes_of(bytes);
        self.items.push((
            id.clone(),
            ItemSource::Call {
                call: range.clone(),
                result: None,
            },
        ));
        self.items.len() - 1
    }

    /// Whether call number `call` already has `range` as its result line.
    pub fn has_result(&self, call: usize, range: &Range<u64>) -> bool {
        matches!(
            self.items.get(call),
            Some((_, ItemSource::Call { result: Some(r), .. })) if r == range
        )
    }

    /// Sets the line of the result of call number `call`.
    pub fn set_result(&mut self, bytes: &ByteSource, call: usize, range: &Range<u64>) {
        self.bytes_of(bytes);
        if let Some((_, ItemSource::Call { result, .. })) = self.items.get_mut(call) {
            replace(result, range);
        }
    }

    /// Sets the row of the prompt (OpenCode).
    pub fn set_own_row(&mut self, row: &str) {
        self.own_row = Some(self.intern(row));
    }

    /// Sets the first assistant row of the turn (OpenCode).
    pub fn set_turn_row(&mut self, row: &str) {
        self.turn_row = Some(self.intern(row));
    }

    /// Adds item `id`, which the caller knows is not in the table yet, as
    /// read from database row `row` (OpenCode). `call`: the item is a tool
    /// call, which lives in a part of the row that has the item's id, so the
    /// call and its result share the one reference.
    pub fn add_row_item(&mut self, id: &ItemId, row: &str, call: bool) {
        let row = self.intern(row);
        self.items.push((id.clone(), ItemSource::Row { row, call }));
    }

    /// Sets the row of item `id`, which is in the table, to `row`: a later
    /// row sent the item again, and the item now comes from that row.
    pub fn set_row_item(&mut self, id: &ItemId, row: &str, call: bool) {
        let row = self.intern(row);
        if let Some((_, source)) = self.items.iter_mut().rfind(|(item, _)| item == id) {
            *source = ItemSource::Row { row, call };
        }
    }

    /// Forgets item `id`, which has left the turn.
    pub fn remove_item(&mut self, id: &ItemId) {
        self.items.retain(|(item, _)| item != id);
    }

    /// `row` as a shared string, sharing the allocation of the one the
    /// latest entry has (the items of a row are added together).
    fn intern(&self, row: &str) -> Arc<str> {
        let known = self.items.last().and_then(|(_, source)| match source {
            ItemSource::Row { row, .. } => Some(row),
            _ => None,
        });
        match known {
            Some(known) if &**known == row => Arc::clone(known),
            _ => Arc::from(row),
        }
    }

    /// The references of section `key`: its line and, for a tool call, the
    /// line of its result; or its row. `None` when the turn has none for it.
    pub fn get(&self, key: &SourceKey) -> Option<SectionRefs> {
        self.row_refs(key)
            .map(SectionRefs::Rows)
            .or_else(|| self.line_refs(key).map(SectionRefs::Lines))
    }

    /// The row references of section `key`, for a turn that was read from
    /// database rows. A footer has only the turn's row.
    fn row_refs(&self, key: &SourceKey) -> Option<RowRefs> {
        let turn = self.turn_row.as_deref().map(str::to_owned);
        let section = match key {
            SourceKey::Own => Some(RowRef {
                row: self.own_row.as_deref()?.to_owned(),
                part: None,
            }),
            SourceKey::Footer => None,
            SourceKey::Item(id) => match &self.items.iter().rfind(|(item, _)| item == id)?.1 {
                ItemSource::Row { row, call } => Some(RowRef {
                    row: row.to_string(),
                    part: call.then(|| id.to_string()),
                }),
                _ => return None,
            },
        };
        (section.is_some() || turn.is_some()).then_some(RowRefs { section, turn })
    }

    fn line_refs(&self, key: &SourceKey) -> Option<LineRefs> {
        let bytes = self.bytes.as_ref()?;
        let reference = |range: &Range<u64>| SourceRef {
            bytes: bytes.clone(),
            range: range.clone(),
        };
        let (line, result) = match key {
            SourceKey::Own => (self.own.as_ref()?, None),
            SourceKey::Footer => (self.footer.as_ref()?, None),
            SourceKey::Item(id) => match &self.items.iter().rfind(|(item, _)| item == id)?.1 {
                ItemSource::Message(message) => {
                    let lines = self.messages.get(*message)?;
                    (lines.stopped.as_ref().unwrap_or(&lines.latest), None)
                }
                ItemSource::Line(range) => (range, None),
                ItemSource::Call { call, result } => (call, result.as_ref()),
                ItemSource::Row { .. } => return None,
            },
        };
        Some(LineRefs {
            line: reference(line),
            result: result.map(reference),
        })
    }
}

/// Sets `slot` to `range`. Returns whether it changed.
fn replace(slot: &mut Option<Range<u64>>, range: &Range<u64>) -> bool {
    if slot.as_ref() == Some(range) {
        false
    } else {
        *slot = Some(range.clone());
        true
    }
}

fn replace_range(slot: &mut Range<u64>, range: &Range<u64>) -> bool {
    if slot == range {
        false
    } else {
        slot.clone_from(range);
        true
    }
}

/// The references of one section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionRefs {
    /// Lines of a transcript (Claude).
    Lines(LineRefs),
    /// Rows of a database (OpenCode).
    Rows(RowRefs),
}

impl SectionRefs {
    /// The lines, for a section read from a transcript.
    pub fn lines(&self) -> Option<&LineRefs> {
        match self {
            Self::Lines(lines) => Some(lines),
            Self::Rows(_) => None,
        }
    }

    /// The rows, for a section read from a database.
    pub fn rows(&self) -> Option<&RowRefs> {
        match self {
            Self::Rows(rows) => Some(rows),
            Self::Lines(_) => None,
        }
    }
}

/// A reference to a row of a database, and to a part of it when the section
/// is only a part of the row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowRef {
    /// The row's id.
    pub row: String,
    /// The id of the part of the row, for a tool call: its call and its
    /// result are one part, so there is one reference for both.
    pub part: Option<String>,
}

/// The references of a section read from a database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowRefs {
    /// The section's own row; `None` for a footer, which has none.
    pub section: Option<RowRef>,
    /// The first assistant row of an Assistant turn, which the Turn group
    /// is read from.
    pub turn: Option<String>,
}

/// The references of a section read from a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRefs {
    /// The section's line: for a message the line that is most complete, for
    /// a tool call the line of the call.
    pub line: SourceRef,
    /// The line of a tool call's result, once it has arrived.
    pub result: Option<SourceRef>,
}

/// What a harness is asked to read about a section: the groups of rows it can
/// give depend on the kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Text, thinking or a notice: the Message group.
    Message,
    /// A tool call: the Tool call group.
    ToolCall,
    /// A user prompt: the Prompt group.
    Prompt,
    /// Any other section: the Environment group only.
    Other,
}

/// A section's references and what it is, as handed to
/// [`Harness::section_details`](crate::harness::Harness::section_details).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionSource {
    /// What the section is.
    pub kind: SourceKind,
    /// Its references.
    pub refs: SectionRefs,
}

/// The groups of rows a harness can read from a source. The model holds the
/// main rows of the `Turn` group; a harness can add to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceGroupKind {
    /// About the turn, beyond what the model knows (OpenCode's provider and
    /// agent).
    Turn,
    /// About one API message.
    Message,
    /// About a tool call.
    ToolCall,
    /// About a user prompt.
    Prompt,
    /// Where the section was recorded.
    Environment,
}

/// A value a harness reads, left for the UI to format (a timestamp in the
/// configured format, a token count the way tokens are shown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceValue {
    /// Text as it is.
    Text(String),
    /// A point in time.
    Time(DateTime<Utc>),
    /// A count of tokens.
    Tokens(u64),
    /// A plain count.
    Count(u64),
}

/// One `label  value` row read from a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    /// The raw field name, or the label the spec gives it.
    pub label: String,
    /// The value.
    pub value: SourceValue,
    /// The label of a row of the model that this row stands in for, when it
    /// is not the same as this row's label (a prompt's `timestamp` is the
    /// source's `time.created`).
    pub replaces: Option<&'static str>,
}

impl SourceRow {
    /// A row.
    pub fn new(label: impl Into<String>, value: SourceValue) -> Self {
        Self {
            label: label.into(),
            value,
            replaces: None,
        }
    }

    /// A row that takes the place of the model's row `replaces`.
    pub fn replacing(label: impl Into<String>, replaces: &'static str, value: SourceValue) -> Self {
        Self {
            replaces: Some(replaces),
            ..Self::new(label, value)
        }
    }
}

/// The rows a harness read for one group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceGroup {
    /// Which group.
    pub kind: SourceGroupKind,
    /// Rows that come before the rows the model knows for the group (a tool
    /// call's `start`).
    pub lead: Vec<SourceRow>,
    /// The other rows, after the model's.
    pub rows: Vec<SourceRow>,
    /// The source could not be read: the group ends with an `unavailable`
    /// row.
    pub unavailable: bool,
}

impl SourceGroup {
    /// A group that could not be read.
    pub fn unavailable(kind: SourceGroupKind) -> Self {
        Self {
            kind,
            lead: Vec::new(),
            rows: Vec::new(),
            unavailable: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes() -> ByteSource {
        ByteSource::Memory(std::sync::Arc::from(&b"0123456789"[..]))
    }

    #[test]
    fn a_message_item_gets_the_latest_stopped_line_else_the_latest() {
        let mut t = TurnSources::new();
        let b = bytes();
        let m = t.add_message(&b, &(0..2));
        let id = ItemId::from("a");
        t.add_message_item(&id, m);
        let line = |t: &TurnSources| {
            t.get(&SourceKey::Item(id.clone()))
                .unwrap()
                .lines()
                .unwrap()
                .line
                .range
                .clone()
        };
        assert_eq!(line(&t), 0..2);
        assert!(t.note_message_line(m, &(3..5), false));
        assert_eq!(line(&t), 3..5);
        assert!(t.note_message_line(m, &(6..8), true));
        assert!(t.note_message_line(m, &(8..9), false));
        assert_eq!(line(&t), 6..8, "the stopped line wins over a later one");
        assert!(!t.note_message_line(m, &(8..9), false), "nothing changed");
    }

    #[test]
    fn a_call_has_two_lines_and_the_latest_entry_of_an_item_wins() {
        let mut t = TurnSources::new();
        let b = bytes();
        let id = ItemId::from("c");
        let at = t.add_call(&b, &id, &(0..2));
        let refs = t.get(&SourceKey::Item(id.clone())).unwrap();
        let refs = refs.lines().unwrap();
        assert_eq!((refs.line.range.clone(), refs.result.clone()), (0..2, None));
        assert!(!t.has_result(at, &(3..4)));
        t.set_result(&b, at, &(3..4));
        assert!(t.has_result(at, &(3..4)));
        let refs = t.get(&SourceKey::Item(id.clone())).unwrap();
        assert_eq!(refs.lines().unwrap().result.clone().unwrap().range, 3..4);
        // An item added again (a notice that was removed and came back) is
        // found by its last entry.
        t.add_line(&b, &id, &(5..6));
        let refs = t.get(&SourceKey::Item(id)).unwrap();
        assert_eq!(refs.lines().unwrap().line.range, 5..6);
    }

    #[test]
    fn a_row_item_has_its_row_and_a_call_its_part_too() {
        let mut t = TurnSources::new();
        t.set_turn_row("a1");
        t.add_row_item(&ItemId::from("a1:0"), "a1", false);
        t.add_row_item(&ItemId::from("call-1"), "a1", true);
        t.set_own_row("u1");
        let rows = |key: SourceKey| t.get(&key).unwrap().rows().unwrap().clone();
        let text = rows(SourceKey::Item(ItemId::from("a1:0")));
        assert_eq!(
            text.section,
            Some(RowRef {
                row: "a1".into(),
                part: None
            })
        );
        assert_eq!(text.turn.as_deref(), Some("a1"));
        let call = rows(SourceKey::Item(ItemId::from("call-1")));
        assert_eq!(call.section.unwrap().part.as_deref(), Some("call-1"));
        assert_eq!(rows(SourceKey::Own).section.unwrap().row, "u1");
        // The footer has the turn's row only.
        let footer = rows(SourceKey::Footer);
        assert_eq!((footer.section, footer.turn.as_deref()), (None, Some("a1")));
        // An item that went has nothing.
        t.remove_item(&ItemId::from("a1:0"));
        assert_eq!(t.get(&SourceKey::Item(ItemId::from("a1:0"))), None);
        assert!(t.get(&SourceKey::Item(ItemId::from("call-1"))).is_some());
    }

    #[test]
    fn a_table_of_lines_has_no_row_references() {
        let mut t = TurnSources::new();
        let b = bytes();
        let (line, call) = (ItemId::from("l"), ItemId::from("c"));
        t.add_line(&b, &line, &(0..2));
        t.add_call(&b, &call, &(3..4));
        t.set_own(&b, &(5..6));
        for key in [SourceKey::Item(line), SourceKey::Item(call), SourceKey::Own] {
            let refs = t.get(&key).unwrap();
            assert!(refs.lines().is_some() && refs.rows().is_none(), "{key:?}");
        }
        assert_eq!(t.get(&SourceKey::Footer), None);
    }

    #[test]
    fn an_item_sent_again_by_another_row_follows_that_row() {
        let mut t = TurnSources::new();
        let id = ItemId::from("call-1");
        t.add_row_item(&id, "a1", true);
        t.add_row_item(&ItemId::from("x"), "a1", false);
        let row = |t: &TurnSources| {
            t.get(&SourceKey::Item(id.clone()))
                .unwrap()
                .rows()
                .unwrap()
                .section
                .clone()
                .unwrap()
                .row
        };
        assert_eq!(row(&t), "a1");
        t.set_row_item(&id, "a2", true);
        assert_eq!(row(&t), "a2");
        assert_eq!(t.items.len(), 2, "replaced, not added");
        // The same row again changes nothing.
        let before = t.clone();
        t.set_row_item(&id, "a2", true);
        assert_eq!(t, before);
    }

    #[test]
    fn an_empty_table_has_no_references() {
        let t = TurnSources::new();
        assert_eq!(t.get(&SourceKey::Own), None);
        assert_eq!(t.get(&SourceKey::Footer), None);
        assert_eq!(t.get(&SourceKey::Item(ItemId::from("x"))), None);
    }
}
