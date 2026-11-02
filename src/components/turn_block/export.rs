//! A session as a markdown document, for the export (SPEC §6.4).
//!
//! [`export_markdown`] is pure: it needs the turns, the fold state and the
//! few settings that decide what is shown ([`ExportSettings`]).
//!
//! The document starts with `# <session title>` and a list of the session's
//! metadata, then has one `## <Role> — <timestamp>` heading per turn
//! followed by its sections, each a paragraph or block of its own:
//!
//! - prompts and assistant text are written as their markdown source;
//! - thinking is a block quote;
//! - a folded section (in the folded export) is its summary, as a block
//!   quote: `▶ Find the BRIO on USB` becomes `> Find the BRIO on USB` (with
//!   `&` and `<` escaped, so that `<dump>` is not read as a tag);
//! - an unfolded tool call (in the folded export) is its description in bold,
//!   then each label in bold with its content in a fenced code block, as it
//!   is displayed (a truncated output ends in its `… N more lines` row);
//! - in the full export every tool call is a `<details>` block holding its
//!   input and its output in fenced code blocks, and nothing is folded,
//!   truncated or hidden.
//!
//! Fences are one backtick longer than the longest run of backticks in their
//! content (and at least three).

use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::plain::{PLAIN_WIDTH, displayed_text};
use super::renderers::{PENDING, SectionCtx, ToolCallSection};
use super::section::{FoldState, Section, SectionContent, derive_sections};
use crate::model::{InjectedEntry, ToolCall, ToolKind, Turn};
use crate::theme::Theme;
use crate::timestamp::TimestampFormatter;

/// What the header of the document says about the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportMeta {
    /// The session title (the first heading).
    pub title: String,
    /// The project's display name.
    pub project: String,
    /// The session id.
    pub session_id: String,
    /// The working directory (empty when unknown).
    pub cwd: String,
    /// When the session started.
    pub started: DateTime<Utc>,
    /// The harness's display name.
    pub harness: String,
}

/// The settings an export depends on.
#[derive(Debug, Clone, Copy)]
pub struct ExportSettings<'a> {
    /// Gives the plain text its styles' shape (never visible in text).
    pub theme: &'a Theme,
    /// Formats the timestamps of the headings.
    pub formatter: &'a TimestampFormatter,
    /// Which sections are folded and which outputs expanded.
    pub folds: &'a FoldState,
    /// `truncate_output_lines`, for the unfolded tool calls of the folded
    /// export.
    pub truncate: usize,
    /// The label of the key that opens a subagent.
    pub open_key: &'a str,
    /// Whether system sections are shown; the folded export leaves them out
    /// when they are not.
    pub show_system: bool,
    /// The width the transcript lays its blocks out at, when known. A folded
    /// summary is cut to what the block shows at that width (a folded prompt
    /// is the two rows of the screen, the second ending in `…`), as the
    /// copy of the section does; without it, at the logical lines.
    pub block_width: Option<u16>,
}

/// The session as markdown, as currently folded (`full` false) or in full.
///
/// `meta` gives the header; without it the document starts at the first
/// turn. The document ends in one newline.
pub fn export_markdown(
    meta: Option<&ExportMeta>,
    turns: &[Arc<Turn>],
    settings: &ExportSettings<'_>,
    full: bool,
) -> String {
    let mut blocks: Vec<String> = Vec::new();
    if let Some(meta) = meta {
        blocks.push(header(meta, settings.formatter));
    }
    for turn in turns {
        blocks.push(heading(turn, settings.formatter));
        let show_system = full || settings.show_system;
        for section in derive_sections(turn, show_system) {
            blocks.extend(section_markdown(turn, &section, settings, full));
        }
    }
    let mut document = blocks.join("\n\n");
    document.push('\n');
    document
}

fn header(meta: &ExportMeta, formatter: &TimestampFormatter) -> String {
    let title = meta.title.trim();
    let title = if title.is_empty() { "Session" } else { title };
    let started = formatter.format(meta.started);
    let cwd = if meta.cwd.is_empty() {
        "(unknown)"
    } else {
        &meta.cwd
    };
    format!(
        "# {title}\n\n- Project: {}\n- Session: {}\n- Working directory: {cwd}\n- Started: {started}\n- Harness: {}",
        meta.project, meta.session_id, meta.harness
    )
}

fn heading(turn: &Turn, formatter: &TimestampFormatter) -> String {
    let time = formatter.format(turn.timestamp());
    match turn {
        Turn::User(_) => format!("## User — {time}"),
        Turn::Assistant(a) => match a.effort.as_deref() {
            Some(effort) => format!("## Assistant ({} @ {effort}) — {time}", a.model),
            None => format!("## Assistant ({}) — {time}", a.model),
        },
        Turn::Compaction(_) => format!("## Compaction — {time}"),
    }
}

/// The markdown of one section, `None` for a section with nothing to write.
fn section_markdown(
    turn: &Turn,
    section: &Section<'_>,
    settings: &ExportSettings<'_>,
    full: bool,
) -> Option<String> {
    let folded = !full && settings.folds.is_folded(section);
    let summary = || {
        displayed_text(
            turn,
            &section.id,
            settings.folds,
            settings.theme,
            settings.truncate,
            settings.open_key,
            settings.block_width,
        )
        .map(|text| quote(&escape_inline(&text)))
    };
    let text = match section.content {
        SectionContent::Prompt(text) | SectionContent::Text(text) => {
            if folded {
                summary()?
            } else {
                text.trim_end().to_owned()
            }
        }
        SectionContent::Thinking(text) => {
            if folded {
                summary()?
            } else {
                quote(text.trim_end())
            }
        }
        SectionContent::Tool(call) => {
            if folded {
                summary()?
            } else if full {
                details(call)
            } else {
                unfolded_tool(call, section, settings)
            }
        }
        SectionContent::ModelChange(_) | SectionContent::Stats(_) => {
            let text = displayed_text(
                turn,
                &section.id,
                settings.folds,
                settings.theme,
                settings.truncate,
                settings.open_key,
                None,
            )?;
            format!("_{text}_")
        }
        SectionContent::Compaction(summary_text) => {
            if folded {
                summary()?
            } else {
                summary_text.trim_end().to_owned()
            }
        }
        SectionContent::System(entries) => {
            if folded {
                summary()?
            } else {
                system_entries(entries)
            }
        }
    };
    Some(text)
}

/// `text` as a block quote.
fn quote(text: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                ">".to_owned()
            } else {
                format!("> {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `content` in a fenced code block with the info string `info`. The fence is
/// longer than any run of backticks in the content.
pub fn fenced(info: &str, content: &str) -> String {
    let fence = "`".repeat(fence_length(content));
    let content = content.strip_suffix('\n').unwrap_or(content);
    format!("{fence}{info}\n{content}\n{fence}")
}

/// The number of backticks of a fence that can hold `content`: one more than
/// its longest run of backticks, at least 3.
pub fn fence_length(content: &str) -> usize {
    let mut longest = 0;
    let mut run = 0;
    for c in content.chars() {
        if c == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    (longest + 1).max(3)
}

/// An unfolded tool call as displayed: the description, then each label with
/// its content fenced.
fn unfolded_tool(call: &ToolCall, section: &Section<'_>, settings: &ExportSettings<'_>) -> String {
    let ctx = SectionCtx {
        theme: settings.theme,
        width: PLAIN_WIDTH,
        folded: false,
        truncate: settings.truncate,
        expanded: settings.folds.is_expanded(&section.id),
        open_key: settings.open_key,
        verbatim: false,
    };
    let parts = ToolCallSection(call).plain_parts(&ctx);
    let description = escape_inline(&parts.description.join(" "));
    let mut blocks = vec![format!("**{description}**")];
    for block in parts.blocks {
        blocks.push(format!("**{}**", block.label));
        let info = if call.tool == ToolKind::Shell && block.label == "Command:" {
            "sh"
        } else {
            ""
        };
        blocks.push(fenced(info, &block.lines.join("\n")));
    }
    blocks.extend(parts.more.map(|more| format!("_{more}_")));
    blocks.join("\n\n")
}

/// A tool call in full, as a `<details>` block: the description in its
/// summary, then the input and the output in fenced code blocks.
fn details(call: &ToolCall) -> String {
    let description = escape_html(&one_line(
        &ToolCallSection(call).description_with_duration(),
    ));
    let (info, input) = match (&call.tool, call.input.get("command")) {
        (ToolKind::Shell, Some(serde_json::Value::String(command))) => ("sh", command.clone()),
        _ => (
            "json",
            serde_json::to_string_pretty(&call.input).unwrap_or_default(),
        ),
    };
    let mut blocks = vec![
        format!("<details><summary>{description}</summary>"),
        fenced(info, &input),
    ];
    match &call.result {
        None => blocks.push(fenced("", PENDING)),
        Some(result) => {
            let output = match result.output.text() {
                Ok(text) => text.into_owned(),
                Err(err) => {
                    log::warn!("could not read the output of {}: {err}", call.id);
                    format!("(output unavailable: {err})")
                }
            };
            blocks.push(fenced("", &output));
        }
    }
    blocks.push("</details>".to_owned());
    blocks.join("\n\n")
}

/// Injected entries: each kind in bold with its text fenced.
fn system_entries(entries: &[InjectedEntry]) -> String {
    entries
        .iter()
        .map(|entry| format!("**{}**\n\n{}", entry.kind, fenced("", &entry.text)))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `text` on one line, its runs of whitespace single spaces.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` with `&` and `<` replaced, so that a renderer shows them as they
/// are (`<dump>` is not an HTML tag) in a summary written as plain text.
fn escape_inline(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;")
}

/// `text` with the characters HTML gives a meaning replaced.
fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests;
