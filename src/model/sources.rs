//! References from a turn's sections to the source they were read from, and
//! what a harness reads there for the details overlay (SPEC cycle 2 §5.2).
//!
//! Turns keep only cheap references (a byte range), never the field values:
//! most of what the overlay shows varies across versions and is rarely looked
//! at, so it is read again from the source when the overlay opens.

use std::io;
use std::ops::Range;
use std::time::Duration;

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
}

/// The references of one turn, by section. A side table, so that the items
/// carry nothing of it. Entries are only added, so the position of an item's
/// entry never changes.
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

    /// Sets the line of the prompt or the compaction. Returns whether the
    /// table changed.
    pub fn set_own(&mut self, bytes: &ByteSource, range: &Range<u64>) -> bool {
        self.bytes_of(bytes);
        replace(&mut self.own, range)
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

    /// Links item `id` to message `message`, unless it is linked already.
    /// Returns whether the table changed.
    pub fn link_message_item(&mut self, id: &ItemId, message: usize) -> bool {
        self.link(id, || ItemSource::Message(message)).1
    }

    /// Links item `id` to one line, unless it is linked already. Returns
    /// whether the table changed.
    pub fn link_line(&mut self, bytes: &ByteSource, id: &ItemId, range: &Range<u64>) -> bool {
        self.bytes_of(bytes);
        self.link(id, || ItemSource::Line(range.clone())).1
    }

    /// Links the tool call `id` to the line of its call, unless it is linked
    /// already. Returns the call's number in the table, to give to
    /// [`set_result`](Self::set_result).
    pub fn link_call(&mut self, bytes: &ByteSource, id: &ItemId, range: &Range<u64>) -> usize {
        self.bytes_of(bytes);
        self.link(id, || ItemSource::Call {
            call: range.clone(),
            result: None,
        })
        .0
    }

    fn link(&mut self, id: &ItemId, new: impl FnOnce() -> ItemSource) -> (usize, bool) {
        if let Some(at) = self.items.iter().rposition(|(item, _)| item == id) {
            return (at, false);
        }
        self.items.push((id.clone(), new()));
        (self.items.len() - 1, true)
    }

    /// Whether call number `call` already has `range` as its result line.
    pub fn has_result(&self, call: usize, range: &Range<u64>) -> bool {
        matches!(
            self.items.get(call),
            Some((_, ItemSource::Call { result: Some(r), .. })) if r == range
        )
    }

    /// Sets the line of the result of call number `call`. Returns whether the
    /// table changed.
    pub fn set_result(&mut self, bytes: &ByteSource, call: usize, range: &Range<u64>) -> bool {
        self.bytes_of(bytes);
        match self.items.get_mut(call) {
            Some((_, ItemSource::Call { result, .. })) => replace(result, range),
            _ => false,
        }
    }

    /// The references of section `key`: its line and, for a tool call, the
    /// line of its result. `None` when the turn has none for it.
    pub fn get(&self, key: &SourceKey) -> Option<SectionRefs> {
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
            },
        };
        Some(SectionRefs {
            line: reference(line),
            result: result.map(reference),
        })
    }

    /// Whether the turn has no references at all.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_none()
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
pub struct SectionRefs {
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

/// The groups of rows a harness can read from a source. The `Turn` group is
/// the model's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceGroupKind {
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
    /// A span of time.
    Duration(Duration),
}

/// One `label  value` row read from a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceRow {
    /// The raw field name, or the label the spec gives it.
    pub label: String,
    /// The value.
    pub value: SourceValue,
}

impl SourceRow {
    /// A row.
    pub fn new(label: impl Into<String>, value: SourceValue) -> Self {
        Self {
            label: label.into(),
            value,
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
        assert!(t.link_message_item(&id, m));
        let line = |t: &TurnSources| t.get(&SourceKey::Item(id.clone())).unwrap().line.range;
        assert_eq!(line(&t), 0..2);
        assert!(t.note_message_line(m, &(3..5), false));
        assert_eq!(line(&t), 3..5);
        assert!(t.note_message_line(m, &(6..8), true));
        assert!(t.note_message_line(m, &(8..9), false));
        assert_eq!(line(&t), 6..8, "the stopped line wins over a later one");
        assert!(!t.note_message_line(m, &(8..9), false), "nothing changed");
    }

    #[test]
    fn a_call_has_two_lines_and_links_are_not_replaced() {
        let mut t = TurnSources::new();
        let b = bytes();
        let id = ItemId::from("c");
        let at = t.link_call(&b, &id, &(0..2));
        assert_eq!(t.link_call(&b, &id, &(5..6)), at, "linked already");
        let refs = t.get(&SourceKey::Item(id.clone())).unwrap();
        assert_eq!((refs.line.range, refs.result), (0..2, None));
        assert!(!t.has_result(at, &(3..4)));
        assert!(t.set_result(&b, at, &(3..4)));
        assert!(t.has_result(at, &(3..4)));
        assert!(!t.set_result(&b, at, &(3..4)));
        let refs = t.get(&SourceKey::Item(id)).unwrap();
        assert_eq!(refs.result.unwrap().range, 3..4);
    }

    #[test]
    fn an_empty_table_has_no_references() {
        let t = TurnSources::new();
        assert!(t.is_empty());
        assert_eq!(t.get(&SourceKey::Own), None);
        assert_eq!(t.get(&SourceKey::Footer), None);
        assert_eq!(t.get(&SourceKey::Item(ItemId::from("x"))), None);
    }
}
