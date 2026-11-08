//! The rounded, titled box that the panes and overlays are drawn in, and the
//! colour of its border.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType};

use crate::theme::{Role, Theme};

/// A rounded block whose border has the colour of `border` and whose title
/// reads `╭─ <title> ─…╮` in `foreground`.
pub(super) fn titled_block(theme: &Theme, title: &str, border: Role) -> Block<'static> {
    let border_style = Style::new().fg(theme.role(border));
    let title_style = Style::new().fg(theme.role(Role::Foreground));
    let title = Line::from(vec![
        Span::styled("─", border_style),
        Span::styled(format!(" {title} "), title_style),
    ]);
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(title)
}

/// The border role of a pane.
pub(super) fn border_role(focused: bool) -> Role {
    if focused { Role::Ring } else { Role::Border }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{buffer_to_text, render};
    use crate::theme::builtin;

    #[test]
    fn the_title_follows_a_dash_in_foreground_and_the_border_is_in_its_role() {
        let theme = builtin::default_theme();
        let buffer = render(20, 3, |frame| {
            frame.render_widget(titled_block(&theme, "Pane", Role::Ring), frame.area());
        });
        let text = buffer_to_text(&buffer);
        assert_eq!(text.lines().next(), Some("╭─ Pane ───────────╮"));
        assert_eq!(text.lines().nth(2), Some("╰──────────────────╯"));
        assert_eq!(buffer[(0, 0)].fg, theme.role(Role::Ring));
        assert_eq!(buffer[(1, 0)].fg, theme.role(Role::Ring));
        assert_eq!(buffer[(3, 0)].fg, theme.role(Role::Foreground));
        assert_eq!(buffer[(0, 1)].fg, theme.role(Role::Ring));
    }

    #[test]
    fn the_border_is_ring_when_focused_and_border_otherwise() {
        assert_eq!(border_role(true), Role::Ring);
        assert_eq!(border_role(false), Role::Border);
    }
}
