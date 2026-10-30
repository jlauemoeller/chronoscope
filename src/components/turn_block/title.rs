//! The top border of a turn block: `╭─ <left title> ─…─ <timestamp> ─╮`.

use ratatui::style::Style;
use ratatui::text::{Line, Span};

use super::sections::ellipsize;
use crate::model::Turn;
use crate::theme::{Role, Theme, Token};
use crate::timestamp::TimestampFormatter;

/// The title line of a turn block.
///
/// The role is on the left (`User`, `Assistant`, `Compaction`), an Assistant
/// block continues with `── <model> @ <effort>` (without ` @ <effort>` when
/// there is no effort), and the timestamp is on the right. When the width
/// does not fit both titles, the timestamp goes first, then the left title is
/// cut with `…`.
#[derive(Debug, Clone, Copy)]
pub struct TitleBar<'a> {
    turn: &'a Turn,
    theme: &'a Theme,
    formatter: &'a TimestampFormatter,
}

impl<'a> TitleBar<'a> {
    /// The title of `turn`, with the timestamp formatted by `formatter`.
    pub fn new(turn: &'a Turn, theme: &'a Theme, formatter: &'a TimestampFormatter) -> Self {
        Self {
            turn,
            theme,
            formatter,
        }
    }

    fn border(&self) -> Style {
        Style::new().fg(self.theme.role(Role::Border))
    }

    fn left(&self) -> Vec<Span<'static>> {
        match self.turn {
            Turn::User(_) => vec![self.title(" User ", Token::UserTitle)],
            Turn::Assistant(a) => {
                let effort = a
                    .effort
                    .as_deref()
                    .map(|e| format!(" @ {e}"))
                    .unwrap_or_default();
                vec![
                    self.title(" Assistant ", Token::AssistantTitle),
                    Span::styled("──", self.border()),
                    self.title(&format!(" {}{effort} ", a.model), Token::AssistantTitle),
                ]
            }
            Turn::Compaction(_) => vec![self.title(" Compaction ", Token::SystemTitle)],
        }
    }

    fn title(&self, text: &str, token: Token) -> Span<'static> {
        Span::styled(text.to_owned(), self.theme.style(token))
    }

    fn right(&self) -> Span<'static> {
        let time = self.formatter.format(self.turn.timestamp());
        self.title(&format!(" {time} "), Token::Timestamp)
    }

    /// The border line for a block `width` cells wide.
    pub fn line(&self, width: u16) -> Line<'static> {
        let width = usize::from(width);
        let border = self.border();
        // `╭─` + left + at least one `─` + right + `─╮`.
        let room = width.saturating_sub(5);
        let mut left = Line::from(self.left());
        let mut right = Some(self.right());
        if left.width() + right.as_ref().map_or(0, Span::width) > room {
            right = None;
        }
        if left.width() > room {
            left = ellipsize(left, room, false);
        }
        let used = left.width() + right.as_ref().map_or(0, Span::width);
        let mut spans = vec![Span::styled("╭─", border)];
        spans.extend(left.spans);
        spans.push(Span::styled(
            "─".repeat(width.saturating_sub(4 + used)),
            border,
        ));
        spans.extend(right);
        spans.push(Span::styled("─╮", border));
        Line::from(spans)
    }
}
