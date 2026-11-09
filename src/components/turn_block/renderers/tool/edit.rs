//! The Edit body: `Path:` and the coloured unified `Diff:`.

use ratatui::style::Style;
use ratatui::text::Line;
use serde_json::Value;
use similar::{ChangeTag, TextDiff};

use super::{
    Block, Body, SectionCtx, ToolBodyRenderer, compact_json, limit, muted_block, path_of,
    text_block, with_status_result,
};
use crate::markdown::char_wrap;
use crate::model::ToolCall;
use crate::theme::Token;

/// How long one diff may take before it settles for a correct but larger one.
const DIFF_DEADLINE: std::time::Duration = std::time::Duration::from_millis(250);

/// Context lines around a change.
const CONTEXT: usize = 3;

/// `Path:` with the path, then `Diff:` with a unified diff (3 context lines)
/// of `old_string` to `new_string`; a MultiEdit lists its edits in turn. `@@`
/// hunk headers are in `muted_foreground`, `+` lines in `diff_added`, `-`
/// lines in `diff_removed` and context lines in `output`. A failed or pending
/// call also shows its `Result:` (error text in `error_output`, or
/// `(pending)`).
#[derive(Debug, Clone, Copy)]
pub struct EditBody;

/// One diff row: its text and the token it is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Header,
    Added,
    Removed,
    Context,
}

/// One `(old, new)` pair of an edit object.
fn pair(edit: &Value) -> Option<(&str, &str)> {
    let get = |keys: [&str; 2]| keys.into_iter().find_map(|k| edit.get(k)?.as_str());
    Some((
        get(["old_string", "oldString"])?,
        get(["new_string", "newString"])?,
    ))
}

/// The `(old, new)` pairs the input holds.
fn edits(input: &Value) -> Vec<(&str, &str)> {
    match input.get("edits").and_then(Value::as_array) {
        Some(list) => list.iter().filter_map(pair).collect(),
        None => pair(input).into_iter().collect(),
    }
}

/// The rows of the unified diffs of `edits`.
fn diff_rows(edits: &[(&str, &str)]) -> Vec<(Kind, String)> {
    let mut rows = Vec::new();
    for (old, new) in edits {
        let diff = TextDiff::configure()
            .timeout(DIFF_DEADLINE)
            .diff_lines(*old, *new);
        for hunk in diff.unified_diff().context_radius(CONTEXT).iter_hunks() {
            rows.push((Kind::Header, hunk.header().to_string()));
            for change in hunk.iter_changes() {
                let (kind, sign) = match change.tag() {
                    ChangeTag::Insert => (Kind::Added, '+'),
                    ChangeTag::Delete => (Kind::Removed, '-'),
                    ChangeTag::Equal => (Kind::Context, ' '),
                };
                let value = change.value().trim_end_matches(['\n', '\r']);
                rows.push((kind, format!("{sign}{value}")));
            }
        }
    }
    rows
}

/// The style a row of `kind` is drawn in.
fn row_style(kind: Kind, ctx: &SectionCtx<'_>) -> Style {
    match kind {
        Kind::Header => ctx.muted(),
        Kind::Added => ctx.theme.style(Token::DiffAdded),
        Kind::Removed => ctx.theme.style(Token::DiffRemoved),
        Kind::Context => ctx.theme.style(Token::Output),
    }
}

/// The diff `rows` in their styles, each wrapped to the body width (or kept
/// whole when the text is verbatim). An empty row is one empty line.
fn diff_lines(rows: &[(Kind, String)], ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (kind, text) in rows {
        let mut wrapped = if ctx.verbatim {
            super::super::verbatim_lines(text, row_style(*kind, ctx))
        } else {
            char_wrap(text, ctx.body_width(), row_style(*kind, ctx))
        };
        if wrapped.is_empty() {
            wrapped.push(Line::default());
        }
        lines.extend(wrapped);
    }
    lines
}

impl ToolBodyRenderer for EditBody {
    fn body(&self, call: &ToolCall, ctx: &SectionCtx<'_>) -> Body {
        let path = match path_of(&call.input) {
            Some(path) => path.to_owned(),
            None => compact_json(&call.input),
        };
        let path_block = text_block("Path:", &path, ctx.theme.style(Token::Command), ctx);
        let rows = diff_rows(&edits(&call.input));
        if rows.is_empty() {
            let body = Body {
                blocks: vec![path_block, muted_block("Diff:", "(no changes)", ctx)],
                ..Body::default()
            };
            return with_status_result(body, call, ctx, false);
        }
        // Whether the diff is cut at the configured limit, expanded or not.
        let content_cut = rows.len() > ctx.truncate;
        let max = limit(ctx);
        let cut = rows.len().saturating_sub(max);
        let lines = diff_lines(&rows[..rows.len().min(max)], ctx);
        let body = Body {
            blocks: vec![
                path_block,
                Block {
                    label: "Diff:",
                    lines,
                },
            ],
            remaining: (cut > 0).then_some(cut),
        };
        with_status_result(body, call, ctx, content_cut)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::fixtures::*;
    use super::super::{SectionRenderer, ToolCallSection, ToolKind, ToolStatus};
    use super::*;
    use crate::theme::Role;

    fn edit(old: &str, new: &str) -> ToolCall {
        call(
            ToolKind::Edit,
            json!({"file_path": "/src/lib.rs", "old_string": old, "new_string": new}),
            Some("The file has been updated."),
        )
    }

    #[test]
    fn a_diff_has_headers_and_coloured_plus_and_minus_lines() {
        let theme = theme();
        let call = edit("a\nb\nc\nd\ne\n", "a\nb\nX\nd\ne\n");
        let rows = section(&call, &theme, 60);
        assert_eq!(
            rows,
            [
                "▼ Do the thing",
                "",
                "  Path:",
                "",
                "     /src/lib.rs",
                "",
                "  Diff:",
                "",
                "     @@ -1,5 +1,5 @@",
                "      a",
                "      b",
                "     -c",
                "     +X",
                "      d",
                "      e",
            ]
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[2], "Path:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(fg_of(&lines[4], "/src"), Some(color(&theme, Role::Accent)));
        assert_eq!(
            fg_of(&lines[6], "Diff:"),
            Some(color(&theme, Role::Warning))
        );
        assert_eq!(
            fg_of(&lines[8], "@@"),
            Some(color(&theme, Role::MutedForeground))
        );
        assert_eq!(fg_of(&lines[9], "a"), Some(color(&theme, Role::Secondary)));
        assert_eq!(fg_of(&lines[11], "-c"), Some(DIFF_REMOVED));
        assert_eq!(fg_of(&lines[12], "+X"), Some(DIFF_ADDED));
    }

    #[test]
    fn a_string_without_a_final_newline_gets_no_marker_row() {
        let theme = theme();
        let call = edit("old text", "new text");
        let rows = section(&call, &theme, 60);
        assert_eq!(
            &rows[8..],
            ["     @@ -1 +1 @@", "     -old text", "     +new text"]
        );
    }

    #[test]
    fn identical_strings_have_no_changes() {
        let theme = theme();
        let call = edit("same", "same");
        let rows = section(&call, &theme, 60);
        assert_eq!(rows[8], "     (no changes)");
    }

    #[test]
    fn a_multi_edit_lists_each_edit_in_turn() {
        let theme = theme();
        let call = call(
            ToolKind::Edit,
            json!({"file_path": "/a", "edits": [
                {"old_string": "one", "new_string": "uno"},
                {"old_string": "two", "new_string": "dos"},
            ]}),
            Some("ok"),
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(
            &rows[8..],
            [
                "     @@ -1 +1 @@",
                "     -one",
                "     +uno",
                "     @@ -1 +1 @@",
                "     -two",
                "     +dos",
            ]
        );
    }

    #[test]
    fn long_diffs_are_cut_at_the_line_limit() {
        let theme = theme();
        let old: String = (0..40).map(|n| format!("l{n}\n")).collect();
        let call = edit(&old, "");
        let ctx = SectionCtx {
            truncate: 5,
            ..SectionCtx::new(&theme, 60, false)
        };
        let body = EditBody.body(&call, &ctx);
        assert_eq!(body.blocks[1].lines.len(), 5);
        // One header and 40 removed lines.
        assert_eq!(body.remaining, Some(41 - 5));
    }

    #[test]
    fn a_failed_edit_shows_its_error_after_the_diff() {
        let theme = theme();
        let mut call = edit("a", "b");
        call.status = ToolStatus::Error;
        call.result.as_mut().unwrap().output =
            crate::model::ToolOutput::inline("String to replace not found".into());
        let rows = section(&call, &theme, 60);
        assert_eq!(
            &rows[8..],
            [
                "     @@ -1 +1 @@",
                "     -a",
                "     +b",
                "",
                "  Result:",
                "",
                "     String to replace not found",
            ]
        );
        let ctx = SectionCtx::new(&theme, 60, false);
        let lines = ToolCallSection(&call).lines(&ctx);
        assert_eq!(
            fg_of(&lines[14], "String"),
            Some(color(&theme, Role::Destructive))
        );
    }

    #[test]
    fn a_pending_edit_shows_pending() {
        let theme = theme();
        let call = call(
            ToolKind::Edit,
            json!({"file_path": "/a", "old_string": "a", "new_string": "b"}),
            None,
        );
        let rows = section(&call, &theme, 60);
        assert_eq!(rows.last().unwrap(), "     (pending)");
    }

    fn labels(body: &Body) -> Vec<&'static str> {
        body.blocks.iter().map(|b| b.label).collect()
    }

    fn at(truncate: usize, expanded: bool, theme: &crate::theme::Theme) -> SectionCtx<'_> {
        SectionCtx {
            truncate,
            expanded,
            ..SectionCtx::new(theme, 60, false)
        }
    }

    fn ten_lines(prefix: &str) -> String {
        (1..=10).map(|n| format!("{prefix}{n}\n")).collect()
    }

    fn failing(old: &str, new: &str, result: Option<&str>, status: ToolStatus) -> ToolCall {
        let mut call = call(
            ToolKind::Edit,
            json!({"file_path": "/a", "old_string": old, "new_string": new}),
            result,
        );
        call.status = status;
        call
    }

    #[test]
    fn failed_short_diff_long_error_puts_the_truncated_result_last() {
        let theme = theme();
        let call = failing("a", "b", Some(&ten_lines("err")), ToolStatus::Error);
        let body = EditBody.body(&call, &at(4, false, &theme));
        assert_eq!(labels(&body), ["Path:", "Diff:", "Result:"]);
        assert_eq!(body.blocks[2].lines.len(), 4);
        assert_eq!(body.remaining, Some(6));
        let body = EditBody.body(&call, &at(4, true, &theme));
        assert_eq!(labels(&body), ["Path:", "Diff:", "Result:"]);
        assert_eq!(body.blocks[2].lines.len(), 10);
        assert_eq!(body.remaining, None);
    }

    #[test]
    fn failed_long_diff_short_error_puts_the_result_before_the_diff() {
        let theme = theme();
        // One header and ten removed lines.
        let call = failing(&ten_lines("l"), "", Some("not found"), ToolStatus::Error);
        for expanded in [false, true] {
            let body = EditBody.body(&call, &at(4, expanded, &theme));
            assert_eq!(labels(&body), ["Path:", "Result:", "Diff:"]);
            assert_eq!(body.blocks[1].lines[0].to_string(), "not found");
            assert_eq!(body.blocks[2].lines.len(), if expanded { 11 } else { 4 });
            assert_eq!(body.remaining, (!expanded).then_some(7));
        }
    }

    #[test]
    fn failed_long_diff_and_long_error_show_the_error_in_full() {
        let theme = theme();
        let call = failing(
            &ten_lines("l"),
            "",
            Some(&ten_lines("err")),
            ToolStatus::Error,
        );
        for expanded in [false, true] {
            let body = EditBody.body(&call, &at(4, expanded, &theme));
            assert_eq!(labels(&body), ["Path:", "Result:", "Diff:"]);
            assert_eq!(body.blocks[1].lines.len(), 10);
            assert_eq!(body.remaining, (!expanded).then_some(7));
        }
    }

    #[test]
    fn pending_long_diff_puts_pending_before_the_diff() {
        let theme = theme();
        let call = failing(&ten_lines("l"), "", None, ToolStatus::Pending);
        for expanded in [false, true] {
            let body = EditBody.body(&call, &at(4, expanded, &theme));
            assert_eq!(labels(&body), ["Path:", "Result:", "Diff:"]);
            assert_eq!(body.blocks[1].lines[0].to_string(), "(pending)");
            assert_eq!(body.remaining, (!expanded).then_some(7));
        }
    }
}
