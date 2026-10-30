//! The system / injected section.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::{
    BODY_INDENT, SectionCtx, SectionRenderer, ellipsize, first_line, fold_marker, indent, label,
    with_lead,
};
use crate::markdown::char_wrap;
use crate::model::InjectedEntry;

/// Injected content, in `muted_foreground`.
///
/// Folded: `▶ <kind>: <first line>` for one entry, `▶ <n> injected items`
/// for several. Unfolded: the same header with `▼`, then each entry's kind
/// line and its full text.
#[derive(Debug, Clone, Copy)]
pub struct SystemSection<'a>(pub &'a [InjectedEntry]);

impl SystemSection<'_> {
    fn header(&self, style: Style) -> Line<'static> {
        let text = match self.0 {
            [one] => {
                let first = first_line(&one.text);
                if first.is_empty() {
                    one.kind.clone()
                } else {
                    format!("{}: {first}", one.kind)
                }
            }
            many => format!("{} injected items", many.len()),
        };
        Line::from(Span::styled(text, style))
    }

    fn entry_lines(entry: &InjectedEntry, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = ctx.muted();
        let mut lines = vec![label(&entry.kind, style)];
        let text = char_wrap(&entry.text, ctx.body_width(), style);
        lines.extend(indent(text, BODY_INDENT, style));
        lines
    }
}

impl SystemSection<'_> {
    /// The section as plain text rows: the header, then (unfolded) each
    /// entry's kind and text, without the fold marker and the column indent.
    pub fn plain_lines(&self, ctx: &SectionCtx<'_>) -> Vec<String> {
        let header = self.header(ctx.muted());
        if ctx.folded {
            return vec![ellipsize(header, usize::from(ctx.width), false).to_string()];
        }
        let mut out = vec![header.to_string()];
        for (i, entry) in self.0.iter().enumerate() {
            if i > 0 {
                out.push(String::new());
            }
            out.push(entry.kind.clone());
            if ctx.verbatim {
                out.extend(entry.text.lines().map(str::to_owned));
            } else {
                out.extend(
                    char_wrap(&entry.text, ctx.body_width(), ctx.muted())
                        .iter()
                        .map(ToString::to_string),
                );
            }
        }
        out
    }
}

impl SectionRenderer for SystemSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        let style = ctx.muted();
        let header = self.header(style);
        if ctx.folded {
            let line = ellipsize(header, usize::from(ctx.width), false);
            return with_lead(vec![line], fold_marker(true), style);
        }
        let mut lines = vec![header];
        for (i, entry) in self.0.iter().enumerate() {
            if i > 0 {
                lines.push(Line::default());
            }
            lines.extend(Self::entry_lines(entry, ctx));
        }
        with_lead(lines, fold_marker(false), style)
    }
}
