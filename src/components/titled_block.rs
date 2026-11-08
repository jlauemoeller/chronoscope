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

/// The most columns the title of a [`titled_block`] `pane_width` columns
/// wide can use: a corner, a dash and a space on either side leave this much
/// of the width.
pub(super) fn title_width(pane_width: u16) -> usize {
    usize::from(pane_width).saturating_sub(6)
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
    fn a_title_of_title_width_columns_fills_the_top_border() {
        assert_eq!(title_width(20), 14);
        assert_eq!(title_width(6), 0);
        assert_eq!(title_width(5), 0);
        assert_eq!(title_width(0), 0);
        let theme = builtin::default_theme();
        let title = "t".repeat(title_width(20));
        let buffer = render(20, 3, |frame| {
            frame.render_widget(titled_block(&theme, &title, Role::Border), frame.area());
        });
        let top = buffer_to_text(&buffer).lines().next().unwrap().to_owned();
        assert_eq!(top, format!("╭─ {title} ─╮"));
    }

    #[test]
    fn the_border_is_ring_when_focused_and_border_otherwise() {
        assert_eq!(border_role(true), Role::Ring);
        assert_eq!(border_role(false), Role::Border);
    }
}
