//! The top border of a turn block: `╭─ <left title> ─…─ <timestamp> ─╮`.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::renderers::ellipsize;
use crate::model::Turn;
use crate::theme::{Role, Theme, Token};
use crate::timestamp::TimestampFormatter;

/// The title line of a turn block.
///
/// The role is on the left (`User`, `Assistant`, `Compaction`), an Assistant
/// block continues with `── <model> @ <effort>` (without ` @ <effort>` when
/// there is no effort), and the timestamp is on the right. When the width
/// does not fit both titles, the model text is cut with `…`, then the
/// timestamp goes, and the left title is cut last.
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

    /// The left title: the fixed part, and the model text of an Assistant
    /// block, which is the part cut first when the width is short.
    fn left(&self) -> (Vec<Span<'static>>, Option<String>) {
        match self.turn {
            Turn::User(_) => (vec![self.title(" User ", Token::UserTitle)], None),
            Turn::Assistant(a) => {
                let effort = a
                    .effort
                    .as_deref()
                    .map(|e| format!(" @ {e}"))
                    .unwrap_or_default();
                (
                    vec![
                        self.title(" Assistant ", Token::AssistantTitle),
                        Span::styled("──", self.border()),
                    ],
                    Some(format!(" {}{effort} ", a.model)),
                )
            }
            Turn::Compaction(_) => (vec![self.title(" Compaction ", Token::SystemTitle)], None),
        }
    }

    fn title(&self, text: &str, token: Token) -> Span<'static> {
        Span::styled(text.to_owned(), self.theme.style(token))
    }

    fn right(&self) -> Span<'static> {
        let time = self.formatter.format(self.turn.timestamp());
        self.title(&format!(" {time} "), Token::Timestamp)
    }

    /// The model text cut with `…` to `width` cells, keeping its leading and
    /// trailing space; `None` when there is no room for at least one letter.
    fn cut_model(&self, text: &str, width: usize) -> Option<Span<'static>> {
        if text.width() <= width {
            return Some(self.title(text, Token::AssistantTitle));
        }
        if width < 4 {
            return None;
        }
        let cut = ellipsize(Line::from(text.trim_end().to_owned()), width - 1, false);
        Some(self.title(&format!("{cut} "), Token::AssistantTitle))
    }

    /// The border line for a block `width` cells wide.
    ///
    /// It is always exactly `width` cells and ends in `╮` (from width 2 on).
    /// When the titles do not fit, the model text is cut with `…` first, the
    /// timestamp is dropped next, and the left title is cut last.
    pub fn line(&self, width: u16) -> Line<'static> {
        let width = usize::from(width);
        let border = self.border();
        // `╭─` + left + at least one `─` + right + `─╮`.
        let room = width.saturating_sub(5);
        if width < 5 || room == 0 {
            let fill = "─".repeat(width.saturating_sub(2));
            return Line::from(Span::styled(format!("╭{fill}╮"), border));
        }
        let (mut left, model) = self.left();
        let prefix: usize = left.iter().map(Span::width).sum();
        let mut right = Some(self.right());
        let model_width = model.as_deref().map_or(0, UnicodeWidthStr::width);
        let right_width = right.as_ref().map_or(0, Span::width);
        if prefix + model_width + right_width > room {
            // Cut the model text so the timestamp still fits, if it can;
            // else drop the timestamp and give the model all the room.
            let cut = model
                .as_deref()
                .and_then(|m| self.cut_model(m, room.saturating_sub(prefix + right_width)));
            let tail = match (&model, cut) {
                (Some(m), None) => {
                    right = None;
                    self.cut_model(m, room.saturating_sub(prefix))
                }
                (_, cut) => cut,
            };
            left.extend(tail);
        } else {
            left.extend(model.map(|m| self.title(&m, Token::AssistantTitle)));
        }
        if left.iter().map(Span::width).sum::<usize>() + right.as_ref().map_or(0, Span::width)
            > room
        {
            right = None;
        }
        let mut left = Line::from(left);
        if left.width() > room {
            left = ellipsize(left, room, false);
        }
        let used = left.width() + right.as_ref().map_or(0, Span::width);
        let mut spans = vec![Span::styled("╭─", border)];
        spans.extend(left.spans);
        spans.push(Span::styled("─".repeat(width - 4 - used), border));
        spans.extend(right);
        spans.push(Span::styled("─╮", border));
        Line::from(spans)
    }
}
