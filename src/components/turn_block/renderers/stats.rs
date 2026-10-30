//! The turn stats footer.

use std::time::Duration;

use ratatui::text::Line;

use super::{SectionCtx, SectionRenderer, with_lead, word_wrap};
use crate::model::TurnStats;

/// The duration of a turn: `12s`, `5m 02s` or `1h 05m`. Sub-second parts are
/// dropped.
pub fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    match secs {
        0..=59 => format!("{secs}s"),
        60..=3599 => format!("{}m {:02}s", secs / 60, secs % 60),
        _ => format!("{}h {:02}m", secs / 3600, secs % 3600 / 60),
    }
}

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

/// The footer text: `<duration> · <n> messages · <tokens> tokens`, with
/// ` · $<cost>` when the cost is known. Unknown parts are left out.
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
    if let Some(tokens) = stats.tokens {
        let unit = if tokens == 1 { "token" } else { "tokens" };
        parts.push(format!("{} {unit}", format_tokens(tokens)));
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
        let text = format_stats(self.0);
        with_lead(word_wrap(&text, ctx.width, ctx.muted()), "  ", ctx.muted())
    }
}
