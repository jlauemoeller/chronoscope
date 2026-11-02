//! Transcript search (SPEC §6.2): the matcher and the search over a snapshot
//! of the turns. Pure functions and plain data; the worker thread that runs
//! it and the `Transcript` that shows the result are elsewhere.
//!
//! # Matching
//!
//! A query is a plain substring with *smart case*: it ignores case unless it
//! contains an upper-case character. Case is folded character by character
//! on the original text, so the byte ranges it reports are always
//! boundaries of the text that was searched, whatever the folding does to
//! the length of a character (`İ`, the Kelvin sign).
//!
//! # What is searched
//!
//! Each section of each turn, as its full *unfolded* plain text (see
//! [`section_text`]): the prompt, assistant text, readable thinking, the tool
//! description as its row draws it (with the agent type of a subagent, the
//! duration, and the tool's name when the description is blank), input and
//! complete output (materialised with
//! `ToolOutput::text()`, so a lazily stored output is read back), system
//! entries, the compaction summary and the stats footer. Sections that are
//! not listed (system content while it is hidden) are not searched.
//!
//! A [`Match`] is a section, the index of the line in the section's text and
//! a byte range within that line. Matches are in display order.

use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

use serde_json::Value;

use crate::components::turn_block::renderers::{ToolCallSection, format_stats};
use crate::components::turn_block::{Section, SectionContent, SectionId, derive_sections};
use crate::model::{InjectedEntry, ToolCall, ToolKind, Turn};

/// A search text with smart case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    text: String,
    /// The characters compared: folded to lower case unless the search is
    /// case-sensitive.
    chars: Vec<char>,
    case_sensitive: bool,
}

impl SearchQuery {
    /// A query for `text`, or `None` when it is empty.
    pub fn new(text: &str) -> Option<Self> {
        if text.is_empty() {
            return None;
        }
        let case_sensitive = text.chars().any(char::is_uppercase);
        let chars = if case_sensitive {
            text.chars().collect()
        } else {
            text.chars().flat_map(char::to_lowercase).collect()
        };
        Some(Self {
            text: text.to_owned(),
            chars,
            case_sensitive,
        })
    }

    /// The text as typed.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The end of the match that starts at byte `start` of `haystack`, which
    /// must be a character boundary.
    fn match_at(&self, haystack: &str, start: usize) -> Option<usize> {
        let mut next = 0;
        for (offset, c) in haystack[start..].char_indices() {
            if self.case_sensitive {
                if self.chars.get(next) != Some(&c) {
                    return None;
                }
                next += 1;
            } else {
                for folded in c.to_lowercase() {
                    if self.chars.get(next) != Some(&folded) {
                        return None;
                    }
                    next += 1;
                }
            }
            if next == self.chars.len() {
                return Some(start + offset + c.len_utf8());
            }
        }
        None
    }

    /// The byte ranges of the occurrences in `haystack`, in order and not
    /// overlapping.
    pub fn find_all(&self, haystack: &str) -> Vec<Range<usize>> {
        let mut found = Vec::new();
        let mut free_from = 0;
        for (start, _) in haystack.char_indices() {
            if start < free_from {
                continue;
            }
            if let Some(end) = self.match_at(haystack, start) {
                found.push(start..end);
                free_from = end;
            }
        }
        found
    }
}

/// One occurrence of the query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    /// The section it is in.
    pub section: SectionId,
    /// The line of the section's plain text (from 0).
    pub line: usize,
    /// The bytes of the occurrence within that line.
    pub range: Range<usize>,
}

/// A snapshot of the turns to search: cheap `Arc<Turn>` clones, and whether
/// system content counts.
#[derive(Clone)]
pub struct SearchCorpus {
    turns: Arc<Vec<Arc<Turn>>>,
    show_system: bool,
}

impl SearchCorpus {
    /// A corpus of `turns`. System content is searched when `show_system`.
    pub fn new(turns: Vec<Arc<Turn>>, show_system: bool) -> Self {
        Self {
            turns: Arc::new(turns),
            show_system,
        }
    }

    /// The turns, in order.
    pub fn turns(&self) -> &[Arc<Turn>] {
        &self.turns
    }

    /// Whether system content is searched.
    pub fn show_system(&self) -> bool {
        self.show_system
    }
}

impl std::fmt::Debug for SearchCorpus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SearchCorpus")
            .field("turns", &self.turns.len())
            .field("show_system", &self.show_system)
            .finish()
    }
}

/// Two corpora are equal when they are the same snapshot.
impl PartialEq for SearchCorpus {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.turns, &other.turns) && self.show_system == other.show_system
    }
}

impl Eq for SearchCorpus {}

/// A search to run: what the `Transcript` hands out for the `App` to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRequest {
    /// Numbers the request; the answer carries it back.
    pub generation: u64,
    /// The query as typed.
    pub query: String,
    /// What to search.
    pub corpus: SearchCorpus,
}

impl SearchRequest {
    /// Runs the search.
    pub fn run(&self) -> SearchMatches {
        let matches = SearchQuery::new(&self.query)
            .map(|query| search(&query, &self.corpus))
            .unwrap_or_default();
        SearchMatches::new(self.generation, matches)
    }
}

/// The answer to a [`SearchRequest`].
///
/// User events must be `Eq + Clone`: the matches are `Arc`-wrapped and two
/// answers are equal when they answer the same request.
#[derive(Debug, Clone)]
pub struct SearchMatches {
    generation: u64,
    matches: Arc<Vec<Match>>,
}

impl SearchMatches {
    /// The `matches` found for the request `generation`.
    pub fn new(generation: u64, matches: Vec<Match>) -> Self {
        Self {
            generation,
            matches: Arc::new(matches),
        }
    }

    /// The number of the request this answers.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The matches, in display order.
    pub fn matches(&self) -> &Arc<Vec<Match>> {
        &self.matches
    }
}

impl PartialEq for SearchMatches {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
    }
}

impl Eq for SearchMatches {}

/// Where the user is among the matches: `current` of `total`, as shown
/// (`3/17`). `current` counts from 1 and is 0 when there is no match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchPosition {
    pub current: usize,
    pub total: usize,
}

impl std::fmt::Display for SearchPosition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.current, self.total)
    }
}

/// Every occurrence of `query` in `corpus`, in display order.
pub fn search(query: &SearchQuery, corpus: &SearchCorpus) -> Vec<Match> {
    let mut matches = Vec::new();
    for turn in corpus.turns() {
        for section in derive_sections(turn, corpus.show_system()) {
            let text = section_text(&section);
            for (line, text) in text.split('\n').enumerate() {
                for range in query.find_all(text) {
                    matches.push(Match {
                        section: section.id.clone(),
                        line,
                        range,
                    });
                }
            }
        }
    }
    matches
}

/// The full unfolded plain text of a section: all that can be read in it,
/// folded or not.
///
/// - Prompt, assistant text, thinking and the compaction summary: the text.
/// - A tool call: the description row as drawn (the description or, when it
///   is blank, the tool's name; then the agent type of a subagent and the
///   duration of a finished call), the string values of the input (not the
///   agent type, which the row has), and the complete output (re-read from the transcript when it is stored lazily;
///   an output that cannot be read is left out).
/// - System content: each entry's kind and text.
/// - The model-change note and the stats footer as they are drawn.
/// - A notice: its text.
pub fn section_text<'a>(section: &Section<'a>) -> Cow<'a, str> {
    match section.content {
        SectionContent::Prompt(text)
        | SectionContent::Text(text)
        | SectionContent::Thinking(text)
        | SectionContent::Compaction(text)
        | SectionContent::Notice(_, text) => Cow::Borrowed(text),
        SectionContent::ModelChange(model) => Cow::Owned(format!("model → {model}")),
        SectionContent::Stats(stats) => Cow::Owned(format_stats(stats)),
        SectionContent::System(entries) => Cow::Owned(system_text(entries)),
        SectionContent::Tool(call) => Cow::Owned(tool_text(call)),
    }
}

fn system_text(entries: &[InjectedEntry]) -> String {
    entries
        .iter()
        .map(|entry| format!("{}\n{}", entry.kind, entry.text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn tool_text(call: &ToolCall) -> String {
    // The description row as drawn, with the agent type of a subagent and
    // the duration after it: the text shown is the text searched.
    let mut parts: Vec<String> = vec![ToolCallSection(call).drawn_description()];
    match (&call.input, agent_type_key(call)) {
        // The agent type is in the description row.
        (Value::Object(map), Some(agent)) => map
            .iter()
            .filter(|(key, _)| key.as_str() != agent)
            .for_each(|(_, value)| input_values(value, &mut parts)),
        (input, _) => input_values(input, &mut parts),
    }
    if let Some(result) = &call.result {
        match result.output.text() {
            Ok(text) => parts.push(text.into_owned()),
            Err(err) => log::warn!("tool output of {} is not searched: {err}", call.id),
        }
    }
    parts.join("\n")
}

/// The key of a subagent's input that gives the agent type shown in the
/// description row (`subagent_type`, else `agent`).
fn agent_type_key(call: &ToolCall) -> Option<&'static str> {
    if call.tool != ToolKind::Subagent {
        return None;
    }
    ["subagent_type", "agent"].into_iter().find(|key| {
        call.input
            .get(*key)
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
    })
}

/// The values of a tool input, without its keys: the strings as they are,
/// the other scalars as JSON. These are what the tool bodies show.
fn input_values(input: &Value, out: &mut Vec<String>) {
    match input {
        Value::String(text) => out.push(text.clone()),
        Value::Array(items) => items.iter().for_each(|item| input_values(item, out)),
        Value::Object(map) => map.values().for_each(|value| input_values(value, out)),
        Value::Null => {}
        other => out.push(other.to_string()),
    }
}

#[cfg(test)]
mod tests;
