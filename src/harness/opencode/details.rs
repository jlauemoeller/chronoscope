//! What the details overlay shows about an OpenCode section (SPEC cycle 2
//! §5.2, §5.3): the fields OpenCode records on the section's
//! `session_message` row.
//!
//! The assembler keeps only the ids of those rows (see
//! [`TurnSources`](crate::model::TurnSources)); the row is read again when the
//! overlay opens, through the read-only [`Db`], and its `data` is parsed then
//! and only then. The fields vary across OpenCode versions, so they are never
//! parsed into the model, and nothing here is shared with the assembler's
//! `data` types except the text of an error, which the notice and the `error`
//! row must say the same.
//!
//! The heads below read what the overlay needs and skip the rest of the row
//! without building a value for it. A field of an unexpected JSON type counts
//! as missing and never fails the row; a row that is gone, or whose `data` is
//! not a JSON object, makes the groups read from it `unavailable`.

use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::assemble::{RowError, error_text, lenient_error};
use super::db::Db;
use crate::harness::shared::{lenient, lenient_object};
use crate::model::{
    RowRef, SectionSource, SourceGroup, SourceGroupKind, SourceKind, SourceRow, SourceValue,
};

/// The groups OpenCode can give for `source`, reading the database at
/// `db_path`: Turn, then Message, Tool call or Prompt by the kind of section,
/// and Environment.
///
/// A database that cannot be opened makes every group unavailable.
pub fn section_details(db_path: &Path, source: &SectionSource) -> Vec<SourceGroup> {
    let db = Db::open(db_path)
        .map_err(|err| log::warn!("could not read the details of a section: {err:#}"))
        .ok();
    read_groups(db.as_ref(), source)
}

/// [`section_details`] on a connection that is already open (`None`: the
/// database could not be opened).
pub(super) fn read_groups(db: Option<&Db>, source: &SectionSource) -> Vec<SourceGroup> {
    // A section read from a transcript is not this harness's.
    let Some(refs) = source.refs.rows() else {
        return Vec::new();
    };
    let read = |row: &str| db.and_then(|db| read_row(db, row));
    // Each distinct row is read once: the section's row is usually the
    // turn's first one.
    let turn_row = refs.turn.as_deref().map(read);
    let shared = |section: &RowRef| refs.turn.as_deref() == Some(section.row.as_str());
    let own_row = refs
        .section
        .as_ref()
        .filter(|section| !shared(section))
        .map(|section| read(&section.row));
    let mut groups = Vec::new();
    if let Some(turn_row) = &turn_row {
        groups.push(turn_group(turn_row.as_ref()));
    }
    if let Some(section) = &refs.section {
        let row = if shared(section) {
            turn_row.as_ref()
        } else {
            own_row.as_ref()
        };
        let row = row.and_then(Option::as_ref);
        match source.kind {
            SourceKind::Message => groups.push(message_group(row)),
            SourceKind::ToolCall => groups.push(tool_group(row, section)),
            SourceKind::Prompt => groups.push(prompt_group(row)),
            SourceKind::Other => {}
        }
        groups.push(environment_group(row));
    }
    groups
}

// -- reading the row

/// A row that was read and whose `data` is an object: what the overlay shows
/// of it, and the `data` for a look at one of its parts.
struct Row {
    id: String,
    head: Head,
    data: String,
}

/// Reads row `id` and parses the head of its `data`; `None` when the row is
/// gone, cannot be read, or its `data` is not a JSON object.
fn read_row(db: &Db, id: &str) -> Option<Row> {
    let data = db
        .select_one(
            "SELECT data FROM session_message WHERE id = ?1",
            [id],
            |row| row.get::<_, String>(0),
        )
        .map_err(|err| log::warn!("could not read session_message {id}: {err:#}"))
        .ok()??;
    // `data` is a JSON object; serde would also read a struct from an array,
    // which is not this format.
    if !data.trim_start().starts_with('{') {
        return None;
    }
    let head = serde_json::from_str(&data)
        .map_err(|err| log::warn!("session_message {id}: details unreadable, {err}"))
        .ok()?;
    Some(Row {
        id: id.to_owned(),
        head,
        data,
    })
}

/// A group made of `rows`; `unavailable` when the row was not read.
fn group(
    kind: SourceGroupKind,
    row: Option<&Row>,
    rows: impl FnOnce(&Row) -> Vec<SourceRow>,
) -> SourceGroup {
    match row {
        Some(row) => SourceGroup {
            kind,
            lead: Vec::new(),
            rows: rows(row),
            unavailable: false,
        },
        None => SourceGroup::unavailable(kind),
    }
}

/// The Turn group beyond the model's rows: `provider` and `agent` of the
/// turn's first assistant row, which follow `cost`.
fn turn_group(row: Option<&Row>) -> SourceGroup {
    group(SourceGroupKind::Turn, row, |row| {
        let mut rows = Rows::default();
        rows.text(
            "provider",
            row.head
                .model
                .as_ref()
                .and_then(|m| m.provider_id.as_deref()),
        );
        rows.text("agent", row.head.agent.as_deref());
        rows.0
    })
}

fn message_group(row: Option<&Row>) -> SourceGroup {
    group(SourceGroupKind::Message, row, |row| {
        let head = &row.head;
        let time = head.time.as_ref();
        let mut rows = Rows::default();
        rows.text("row id", Some(&row.id));
        rows.time("time.created", time.and_then(|t| t.created));
        rows.time("time.streamed", time.and_then(|t| t.streamed));
        rows.time("time.completed", time.and_then(|t| t.completed));
        rows.text("finish", head.finish.as_deref());
        rows.text("rawFinish", head.raw_finish.as_deref());
        // The text of the notice for the same row. An error that says
        // nothing is a row only where the notice says `error` for it.
        let ended_in_error = head.finish.as_deref().map(str::trim) == Some("error");
        let error = head
            .error
            .as_ref()
            .filter(|error| ended_in_error || error.has_text())
            .map(|error| error_text(Some(error)).1);
        rows.text("error", error.as_deref());
        rows.0
    })
}

/// The Tool call group: `start`, the part's `time.created`, leads. A part
/// that is not in the row any more makes the group unavailable.
fn tool_group(row: Option<&Row>, section: &RowRef) -> SourceGroup {
    let part = row.and_then(|row| find_part(row, section.part.as_deref()?));
    let Some(part) = part else {
        return SourceGroup::unavailable(SourceGroupKind::ToolCall);
    };
    let mut lead = Rows::default();
    lead.time("start", part.time.as_ref().and_then(|t| t.created));
    SourceGroup {
        kind: SourceGroupKind::ToolCall,
        lead: lead.0,
        rows: Vec::new(),
        unavailable: false,
    }
}

/// The user prompt's group. Its `time.created` stands in for the model's
/// `timestamp`.
fn prompt_group(row: Option<&Row>) -> SourceGroup {
    group(SourceGroupKind::Prompt, row, |row| {
        let created = row.head.time.as_ref().and_then(|t| t.created);
        let mut rows = Rows::default();
        rows.push(
            created
                .and_then(time_value)
                .map(|time| SourceRow::replacing("time.created", "timestamp", time)),
        );
        rows.0
    })
}

fn environment_group(row: Option<&Row>) -> SourceGroup {
    group(SourceGroupKind::Environment, row, |row| {
        let snapshot = row.head.snapshot.as_ref();
        let mut rows = Rows::default();
        rows.text("snapshot.start", snapshot.and_then(|s| s.start.as_deref()));
        rows.text("snapshot.end", snapshot.and_then(|s| s.end.as_deref()));
        rows.0
    })
}

/// The tool part of the row that the item `part` shows, the way the assembler
/// reads it: only a part of type `tool` is a tool call, it is named by its own
/// `id`, or `<row id>:<index>` when it has none, and of several parts with one
/// id the last wins.
fn find_part(row: &Row, part: &str) -> Option<PartHead> {
    let content: ToolHead = serde_json::from_str(&row.data)
        .map_err(|err| log::warn!("session_message {}: parts unreadable, {err}", row.id))
        .ok()?;
    content
        .content
        .unwrap_or_default()
        .iter()
        .enumerate()
        .rev()
        // A part that is not an object is no tool part, but it has a place.
        .filter(|(_, raw)| raw.get().starts_with('{'))
        .filter_map(|(index, raw)| Some((index, serde_json::from_str::<PartHead>(raw.get()).ok()?)))
        .filter(|(_, p)| p.kind.as_deref() == Some("tool"))
        .find(
            |(index, p)| match p.id.as_deref().filter(|id| !id.is_empty()) {
                Some(id) => id == part,
                None => part == format!("{}:{index}", row.id),
            },
        )
        .map(|(_, part)| part)
}

// -- the rows

/// Rows being collected: a field that is missing adds none.
#[derive(Default)]
struct Rows(Vec<SourceRow>);

impl Rows {
    fn push(&mut self, row: Option<SourceRow>) {
        self.0.extend(row);
    }

    /// A text field, trimmed; a blank string is no value.
    fn text(&mut self, label: &str, value: Option<&str>) {
        self.push(
            value
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(|v| SourceRow::new(label, SourceValue::Text(v.to_owned()))),
        );
    }

    /// A time in Unix milliseconds; one that is out of range is no value.
    fn time(&mut self, label: &str, millis: Option<i64>) {
        self.push(
            millis
                .and_then(time_value)
                .map(|time| SourceRow::new(label, time)),
        );
    }
}

fn time_value(millis: i64) -> Option<SourceValue> {
    DateTime::<Utc>::from_timestamp_millis(millis).map(SourceValue::Time)
}

// -- the typed heads

/// The fields of a `data` the overlay can show. Everything else, among it
/// the content, is skipped.
#[derive(Deserialize)]
struct Head {
    #[serde(default, deserialize_with = "lenient_object")]
    time: Option<TimeHead>,
    #[serde(default, deserialize_with = "lenient")]
    agent: Option<String>,
    #[serde(default, deserialize_with = "lenient_object")]
    model: Option<ModelHead>,
    #[serde(default, deserialize_with = "lenient")]
    finish: Option<String>,
    #[serde(default, rename = "rawFinish", deserialize_with = "lenient")]
    raw_finish: Option<String>,
    #[serde(default, deserialize_with = "lenient_error")]
    error: Option<RowError>,
    #[serde(default, deserialize_with = "lenient_object")]
    snapshot: Option<SnapshotHead>,
}

#[derive(Deserialize)]
struct TimeHead {
    #[serde(default, deserialize_with = "lenient")]
    created: Option<i64>,
    #[serde(default, deserialize_with = "lenient")]
    streamed: Option<i64>,
    #[serde(default, deserialize_with = "lenient")]
    completed: Option<i64>,
}

#[derive(Deserialize)]
struct ModelHead {
    #[serde(default, rename = "providerID", deserialize_with = "lenient")]
    provider_id: Option<String>,
}

#[derive(Deserialize)]
struct SnapshotHead {
    #[serde(default, deserialize_with = "lenient")]
    start: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    end: Option<String>,
}

/// The `content` of a row, as far as finding a tool part goes. A part that
/// is not an object is skipped, but counts for the position of the others.
#[derive(Deserialize)]
struct ToolHead {
    content: Option<Vec<Box<serde_json::value::RawValue>>>,
}

/// A part of `content`. The `state` (the tool's input and output) is
/// skipped.
#[derive(Deserialize)]
struct PartHead {
    #[serde(default, rename = "type", deserialize_with = "lenient")]
    kind: Option<String>,
    #[serde(default, deserialize_with = "lenient")]
    id: Option<String>,
    #[serde(default, deserialize_with = "lenient_object")]
    time: Option<TimeHead>,
}
