//! The markdown and code renderer (product §4.5).
//!
//! [`render_markdown`] turns markdown into styled, wrapped Ratatui lines for
//! a width, a base style and a theme. It works in two steps: [`blocks`] parses
//! the source into a small block tree, and [`layout`] lays the tree out. Prose
//! is word-wrapped ([`prose`]); code and tool output are char-wrapped
//! ([`wrap`]); fenced code is highlighted with syntect ([`highlight`]) and
//! mapped to the theme's `syntax.*` tokens. The [`Markdown`] widget draws the
//! result into a buffer.
//!
//! Every colour and modifier comes from the theme tokens or is derived from
//! the base style; nothing here names a colour.

mod blocks;
mod highlight;
mod layout;
mod prose;
mod table;
mod widget;
mod wrap;

#[cfg(test)]
mod tests;

use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use crate::theme::{Role, Theme, Token};

pub use highlight::highlight_code;
pub use widget::Markdown;
pub use wrap::{char_wrap, wrap_styled_lines};

/// The styles the renderer derives from the base style and the theme.
#[derive(Clone, Copy)]
pub(crate) struct Styles<'a> {
    /// The theme, for the syntax tokens.
    pub theme: &'a Theme,
    /// The section's text style; every element style is derived from it.
    pub base: Style,
    /// The `command` token, patched onto the surrounding style for inline code.
    pub command: Style,
    /// The base style with the `muted_foreground` colour, for quote bars,
    /// table separators and rules.
    pub muted: Style,
}

impl<'a> Styles<'a> {
    fn new(base: Style, theme: &'a Theme) -> Self {
        Self {
            theme,
            base,
            command: theme.style(Token::Command),
            muted: base.fg(theme.role(Role::MutedForeground)),
        }
    }

    /// The style of a heading: bold, and underlined too at level 1.
    pub(crate) fn heading(&self, level: u8) -> Style {
        let style = self.base.add_modifier(Modifier::BOLD);
        if level == 1 {
            style.add_modifier(Modifier::UNDERLINED)
        } else {
            style
        }
    }
}

/// Renders markdown `src` into lines no wider than `width` cells.
///
/// That holds down to a minimum width: a quote bar, a list marker or nesting
/// and a code indent each need their own width plus one cell of text, and a
/// table needs one cell per column plus its separators. Below that the lines
/// are wider than `width` (a widget clips them). With a `width` of 0 there
/// are no lines.
///
/// `base` is the section's text style (e.g. `foreground` for assistant text,
/// `thinking` for thinking). The result is pure and deterministic: blocks are
/// separated by one blank line and there is no trailing blank line.
pub fn render_markdown(src: &str, width: u16, base: Style, theme: &Theme) -> Vec<Line<'static>> {
    if width == 0 {
        return Vec::new();
    }
    let styles = Styles::new(base, theme);
    let tree = blocks::parse(src, &styles);
    layout::layout(&tree, usize::from(width), &styles)
}
