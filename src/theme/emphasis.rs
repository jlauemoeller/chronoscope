//! The fixed emphasis styles of product §7.2 and §4.5.
//!
//! No theme token carries bold, reverse video, italic, underline or
//! strikethrough: the product fixes them. This module gives each of those
//! styles a name and, where product §7.2 joins a modifier to a colour, builds
//! the whole style from the theme passed in. It is the one place outside
//! `src/theme/` that production code takes a modifier from; components,
//! renderers and the markdown renderer never write one themselves.

use ratatui::style::{Modifier, Style};

use super::{Role, Theme, Token};

/// The patch that marks the selected transcript section (product §7.2): bold
/// over the section's own token colours.
///
/// It covers every span of the section the turn block emphasises, including
/// the hint of a subagent title row, which is bold when the section is
/// selected (product §4.2, §7.2). The row's rule is left out by the turn
/// block, so it is never bold.
pub const fn selected_section() -> Style {
    Style::new().add_modifier(Modifier::BOLD)
}

/// The label of the selected theme-selector row (product §7.2, §7.4):
/// `foreground`, bold, not inverted.
pub fn selected_theme_row(theme: &Theme) -> Style {
    bold_foreground(theme)
}

/// The group headings of the help overlay (product §3.3): `foreground`, bold.
pub fn help_heading(theme: &Theme) -> Style {
    bold_foreground(theme)
}

/// The group headings of the details overlay (product §3.3, "under bold
/// group headings"): `foreground`, bold.
pub fn details_heading(theme: &Theme) -> Style {
    bold_foreground(theme)
}

/// The project name in the browser header (product §3.2): `foreground`, bold.
pub fn header_project(theme: &Theme) -> Style {
    bold_foreground(theme)
}

fn bold_foreground(theme: &Theme) -> Style {
    Style::new()
        .fg(theme.role(Role::Foreground))
        .add_modifier(Modifier::BOLD)
}

/// The styles of the selected row of a list (the session selector's Projects
/// and Sessions lists) or of the turns sidebar (product §7.2, §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection {
    /// The `>` or `▒` marker.
    pub marker: Style,
    /// Everything after the marker.
    pub body: Style,
}

impl Selection {
    /// The selected row in the list or sidebar that has focus or not
    /// (product §7.2).
    ///
    /// With focus, the marker takes the `selection_marker` token, never
    /// inverted, and the body is one inverted band: `foreground` with reverse
    /// video, not bold. Without focus, the body is bold
    /// `secondary_foreground`, not inverted, and the marker the same colour,
    /// not bold.
    pub fn new(theme: &Theme, focused: bool) -> Self {
        if focused {
            Self {
                marker: theme.style(Token::SelectionMarker),
                body: Style::new()
                    .fg(theme.role(Role::Foreground))
                    .add_modifier(Modifier::REVERSED),
            }
        } else {
            let secondary = Style::new().fg(theme.role(Role::SecondaryForeground));
            Self {
                marker: secondary,
                body: secondary.add_modifier(Modifier::BOLD),
            }
        }
    }
}

/// The label of the selected harness tab (product §7.2): `primary`. Its
/// underline is the row below, drawn with [`harness_tab_underline`], not an
/// underline modifier.
pub fn harness_tab(theme: &Theme) -> Style {
    Style::new().fg(theme.role(Role::Primary))
}

/// The underline row under the selected harness tab (product §7.2): a row of
/// glyphs in `primary`.
pub fn harness_tab_underline(theme: &Theme) -> Style {
    Style::new().fg(theme.role(Role::Primary))
}

/// Markdown strong text (product §4.5): the surrounding style, bold.
pub fn strong(outer: Style) -> Style {
    outer.add_modifier(Modifier::BOLD)
}

/// Markdown emphasis (product §4.5): the surrounding style, italic.
pub fn emphasis(outer: Style) -> Style {
    outer.add_modifier(Modifier::ITALIC)
}

/// Markdown strikethrough (product §4.5): the surrounding style, crossed out.
pub fn strikethrough(outer: Style) -> Style {
    outer.add_modifier(Modifier::CROSSED_OUT)
}

/// Markdown link text (product §4.5): the surrounding style, underlined.
pub fn link(outer: Style) -> Style {
    outer.add_modifier(Modifier::UNDERLINED)
}

/// A markdown heading of `level` (product §4.5): the base style, bold, and
/// underlined too at level 1.
pub fn heading(base: Style, level: u8) -> Style {
    let style = base.add_modifier(Modifier::BOLD);
    if level == 1 {
        style.add_modifier(Modifier::UNDERLINED)
    } else {
        style
    }
}

/// The header row of a markdown table (product §4.5): the base style, bold.
pub fn table_header(base: Style) -> Style {
    base.add_modifier(Modifier::BOLD)
}

#[cfg(test)]
mod tests {
    use ratatui::style::Color;

    use super::*;
    use crate::theme::TokenOverride;

    fn theme() -> Theme {
        Theme::default()
    }

    #[test]
    fn the_selected_section_is_bold_with_no_colour() {
        assert_eq!(
            selected_section(),
            Style::new().add_modifier(Modifier::BOLD)
        );
    }

    #[test]
    fn the_bold_labels_are_bold_foreground() {
        let theme = theme();
        let expected = Style::new()
            .fg(theme.role(Role::Foreground))
            .add_modifier(Modifier::BOLD);
        assert_eq!(selected_theme_row(&theme), expected);
        assert_eq!(help_heading(&theme), expected);
        assert_eq!(details_heading(&theme), expected);
        assert_eq!(header_project(&theme), expected);
    }

    #[test]
    fn the_focused_selection_is_a_band_of_reversed_foreground_that_is_not_bold() {
        let theme = theme();
        let selection = Selection::new(&theme, true);
        assert_eq!(selection.marker, theme.style(Token::SelectionMarker));
        assert_eq!(selection.body.fg, Some(theme.role(Role::Foreground)));
        assert!(selection.body.add_modifier.contains(Modifier::REVERSED));
        assert!(!selection.body.add_modifier.contains(Modifier::BOLD));
        assert!(!selection.marker.add_modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn the_focused_marker_follows_an_overridden_selection_marker() {
        let red = Color::Rgb(255, 0, 0);
        let theme = theme().with_token(Token::SelectionMarker, TokenOverride::fg(red));
        assert_eq!(Selection::new(&theme, true).marker.fg, Some(red));
    }

    #[test]
    fn the_unfocused_selection_is_bold_secondary_foreground_with_a_plain_marker() {
        let theme = theme();
        let secondary = Style::new().fg(theme.role(Role::SecondaryForeground));
        let selection = Selection::new(&theme, false);
        assert_eq!(selection.marker, secondary);
        assert_eq!(selection.body, secondary.add_modifier(Modifier::BOLD));
    }

    #[test]
    fn the_harness_tab_and_its_underline_row_are_primary_without_an_underline_modifier() {
        let theme = theme();
        let primary = Style::new().fg(theme.role(Role::Primary));
        assert_eq!(harness_tab(&theme), primary);
        assert_eq!(harness_tab_underline(&theme), primary);
    }

    #[test]
    fn markdown_inline_styles_add_their_modifier_to_the_outer_style() {
        let outer = Style::new().fg(Color::Red).add_modifier(Modifier::BOLD);
        assert_eq!(strong(outer), outer);
        assert_eq!(emphasis(outer), outer.add_modifier(Modifier::ITALIC));
        assert_eq!(
            strikethrough(outer),
            outer.add_modifier(Modifier::CROSSED_OUT)
        );
        assert_eq!(link(outer), outer.add_modifier(Modifier::UNDERLINED));
        let plain = Style::new().fg(Color::Red);
        assert_eq!(strong(plain), outer);
    }

    #[test]
    fn only_a_level_one_heading_is_underlined() {
        let base = Style::new().fg(Color::Blue);
        let bold = base.add_modifier(Modifier::BOLD);
        assert_eq!(heading(base, 1), bold.add_modifier(Modifier::UNDERLINED));
        for level in 2..=6 {
            assert_eq!(heading(base, level), bold, "level {level}");
        }
    }

    #[test]
    fn a_table_header_is_the_base_style_bold() {
        let base = Style::new().fg(Color::Green);
        assert_eq!(table_header(base), base.add_modifier(Modifier::BOLD));
    }
}
