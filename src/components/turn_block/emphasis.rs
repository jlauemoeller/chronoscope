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
pub fn emphasize(
    lines: Vec<Line<'static>>,
    selected: bool,
    dim_unselected: bool,
    theme: &Theme,
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
        .map(|mut line| {
            for span in &mut line.spans {
                span.style = span.style.patch(patch);
            }
            line
        })
        .collect()
}
