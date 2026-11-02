//! The turn stats footer.

use ratatui::text::Line;

use super::{SectionCtx, SectionRenderer, with_lead, word_wrap};
use crate::duration::format_duration;
use crate::model::TurnStats;

/// A token count: `950`, `45.2k` or `1.2M`.
pub fn format_tokens(tokens: u64) -> String {
    if tokens < 1_000 {
        return tokens.to_string();
    }
    let thousands = tokens.saturating_add(50) / 100;
    if thousands < 10_000 {
        return format!("{}.{}k", thousands / 10, thousands % 10);
    }
    let millions = tokens.saturating_add(50_000) / 100_000;
    format!("{}.{}M", millions / 10, millions % 10)
}

/// The footer text: `<duration> · <n> messages · <tokens> tokens ·
/// <cached> cached`, with ` · $<cost>` last when the cost is known. Unknown
/// parts are left out, and `cached` is hidden when no tokens were read from
/// the cache.
pub fn format_stats(stats: &TurnStats) -> String {
    let mut parts = vec![format_duration(stats.duration)];
    if stats.message_count > 0 {
        let unit = if stats.message_count == 1 {
            "message"
        } else {
            "messages"
        };
        parts.push(format!("{} {unit}", stats.message_count));
    }
    if let Some(usage) = &stats.tokens {
        let tokens = usage.total();
        let unit = if tokens == 1 { "token" } else { "tokens" };
        parts.push(format!("{} {unit}", format_tokens(tokens)));
        if usage.cache_read > 0 {
            parts.push(format!("{} cached", format_tokens(usage.cache_read)));
        }
    }
    if let Some(cost) = stats.cost {
        parts.push(format!("${cost:.2}"));
    }
    parts.join(" · ")
}

/// The muted one-line footer of an Assistant turn.
#[derive(Debug, Clone, Copy)]
pub struct StatsSection<'a>(pub &'a TurnStats);

impl SectionRenderer for StatsSection<'_> {
    fn lines(&self, ctx: &SectionCtx<'_>) -> Vec<Line<'static>> {
        // When a part and its separator fit on a row, a row never starts
        // with the separator: it stays at the end of the row, glued to the
        // part before it by a no-break space that is put back to a plain
        // space once the rows are made. `format_stats` is unchanged, because
        // search matches against its text.
        let text = format_stats(self.0).replace(" · ", "\u{a0}· ");
        let lines = word_wrap(&text, ctx.width, ctx.muted())
            .into_iter()
            .map(|line| {
                let spans = line.spans.into_iter().map(|mut span| {
                    span.content = span.content.replace('\u{a0}', " ").into();
                    span
                });
                Line::from(spans.collect::<Vec<_>>())
            })
            .collect();
        with_lead(lines, "  ", ctx.muted())
    }
}
