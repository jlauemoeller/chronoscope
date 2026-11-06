//! Selected versus unselected text (SPEC §8.2).

use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use crate::theme::{Role, Theme};

/// Applies the emphasis of one section to its lines.
///
/// The selected section keeps its token colours and becomes bold. Any other
/// section keeps them too, unless `dim_unselected` is set: then every
/// foreground becomes `muted_foreground` (modifiers are kept). Borders and
/// titles are not passed through here, so they are never dimmed.
///
/// `keep` is the `(row, span)` of a span that is left as it is: the rule of a
/// subagent's title row, which is always `border`, never bold.
pub fn emphasize(
    lines: Vec<Line<'static>>,
    selected: bool,
    dim_unselected: bool,
    theme: &Theme,
    keep: Option<(usize, usize)>,
) -> Vec<Line<'static>> {
    let patch = if selected {
        Style::new().add_modifier(Modifier::BOLD)
    } else if dim_unselected {
        Style::new().fg(theme.role(Role::MutedForeground))
    } else {
        return lines;
    };
    lines
        .into_iter()
        .enumerate()
        .map(|(row, mut line)| {
            for (index, span) in line.spans.iter_mut().enumerate() {
                if keep != Some((row, index)) {
                    span.style = span.style.patch(patch);
                }
            }
            line
        })
        .collect()
}
