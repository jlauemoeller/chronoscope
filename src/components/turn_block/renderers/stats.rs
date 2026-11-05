//! The turn stats footer.

use ratatui::text::Line;

use super::{SectionCtx, SectionRenderer, with_lead, word_wrap};
use crate::duration::format_duration;
use crate::model::TurnStats;
use crate::tokens::format_tokens;

/// A cost in US dollars: `$0.42`.
pub fn format_cost(cost: f64) -> String {
    format!("${cost:.2}")
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
        parts.push(format_cost(cost));
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
