//! The description row of a tool call: the description, the agent type of a
//! subagent and the muted duration after them, and the rule and open hint
//! that a subagent's title row adds to them.

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use super::super::{PADDING_REACH, TitleChrome, ellipsize, word_wrap};

/// What stays at the end of a folded row when the description is cut.
#[derive(Debug, Clone, Copy)]
pub(super) struct Tail<'a> {
    /// ` (agent)`, in the style of the description.
    pub agent: Option<&'a str>,
    /// ` · 1.2s`, muted.
    pub duration: Option<&'a str>,
}

impl Tail<'_> {
    fn width(&self) -> usize {
        [self.agent, self.duration]
            .into_iter()
            .flatten()
            .map(UnicodeWidthStr::width)
            .sum()
    }
}

/// The folded row: `description` is cut with `…` first and the `tail` stays.
/// When the tail leaves no room for the description (it needs at least one
/// cell and the `…`), the whole row is cut instead.
pub(super) fn folded(
    description: &str,
    tail: Tail<'_>,
    width: usize,
    style: Style,
    muted: Style,
) -> Line<'static> {
    let tail_width = tail.width();
    if tail_width == 0 {
        let line = Line::from(Span::styled(description.to_owned(), style));
        return ellipsize(line, width, false);
    }
    let mut tail_spans = Vec::new();
    if let Some(agent) = tail.agent {
        tail_spans.push(Span::styled(agent.to_owned(), style));
    }
    if let Some(duration) = tail.duration {
        tail_spans.push(Span::styled(duration.to_owned(), muted));
    }
    if tail_width + 1 >= width {
        let mut spans = vec![Span::styled(description.to_owned(), style)];
        spans.extend(tail_spans);
        return cut(Line::from(spans), width);
    }
    let line = Line::from(Span::styled(description.to_owned(), style));
    let mut line = ellipsize(line, width - tail_width, false);
    line.spans.extend(tail_spans);
    line
}

/// `line` cut to `width` cells. The `…` takes the style of the text it
/// replaces, not that of the last span of the line.
fn cut(line: Line<'static>, width: usize) -> Line<'static> {
    if line.width() <= width {
        return line;
    }
    let mut cut = ellipsize(line, width, false);
    if let Some(style) = cut
        .spans
        .len()
        .checked_sub(2)
        .map(|before| cut.spans[before].style)
        && let Some(ellipsis) = cut.spans.last_mut()
    {
        ellipsis.style = style;
    }
    cut
}

/// The unfolded rows: `text` word-wrapped at `width`, and the muted
/// `duration` (` · 1.2s`) after it. The duration is kept in one piece: it
/// goes on the last row when it fits there, else on a row of its own
/// (`· 1.2s`), and is broken only when it is wider than a row.
pub(super) fn wrapped(
    text: &str,
    duration: Option<&str>,
    width: u16,
    style: Style,
    muted: Style,
) -> Vec<Line<'static>> {
    let mut rows = word_wrap(text, width, style);
    let Some(duration) = duration else {
        return rows;
    };
    let room = usize::from(width.max(1));
    let alone = duration.trim_start();
    let last = rows.last_mut().expect("word_wrap gives a row");
    if last.width() + duration.width() <= room {
        last.spans.push(Span::styled(duration.to_owned(), muted));
    } else if alone.width() <= room {
        rows.push(Line::from(Span::styled(alone.to_owned(), muted)));
    } else {
        rows.extend(word_wrap(alone, width, muted));
    }
    rows
}

/// The blank cells on each side of the rule.
const GAP: usize = 2;
/// The fewest cells of a rule.
const MIN_RULE: usize = 3;

/// What a title row has room for after its text (SPEC cycle 4 §2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Trailer {
    /// Neither rule nor hint: the row is as it was.
    Nothing,
    /// The hint is left out; the gap and the rule run to the row's end.
    Rule { rule: usize },
    /// A gap, the rule, a gap and the hint, ending at the row's end.
    Full { rule: usize },
}

impl Trailer {
    /// The trailer of a row `row` cells wide whose text is `text` cells wide,
    /// for a hint of `hint` cells. The rule is at least [`MIN_RULE`] cells and
    /// shrinks first; the hint is left out before the rule.
    pub(super) fn fit(text: usize, row: usize, hint: usize) -> Self {
        let rule_only = text + GAP + MIN_RULE;
        let with_hint = rule_only + GAP + hint;
        if row >= with_hint {
            Self::Full {
                rule: row - text - 2 * GAP - hint,
            }
        } else if row >= rule_only {
            Self::Rule {
                rule: row - text - GAP,
            }
        } else {
            Self::Nothing
        }
    }
}

/// The hint on a row of its own, from the text column: the rule and the hint
/// when `row` cells hold them, else the hint alone.
fn own_row_rule(row: usize, hint: usize) -> Option<usize> {
    (row >= hint + GAP + MIN_RULE).then(|| row - hint - GAP)
}

/// The rule and the hint of a subagent's title row (SPEC cycle 4 §2.2).
#[derive(Debug, Clone, Copy)]
pub(super) struct OpenHint<'a> {
    /// `<key> to open transcript`.
    pub text: &'a str,
    /// The style of the rule: `border`, whatever the emphasis.
    pub rule: Style,
    /// The style of the hint: muted.
    pub hint: Style,
}

impl OpenHint<'_> {
    fn width(&self) -> usize {
        self.text.width()
    }

    fn blank(cells: usize) -> Span<'static> {
        Span::raw(" ".repeat(cells))
    }

    fn rule_span(&self, cells: usize) -> Span<'static> {
        Span::styled("─".repeat(cells), self.rule)
    }

    fn hint_span(&self) -> Span<'static> {
        Span::styled(self.text.to_owned(), self.hint)
    }

    /// Appends a gap and a rule of `rule` cells to `line`, and a gap and the
    /// hint when `with_hint`. Returns the index of the rule's span.
    fn extend(&self, line: &mut Line<'static>, rule: usize, with_hint: bool) -> usize {
        let at = line.spans.len() + 1;
        line.spans.extend([Self::blank(GAP), self.rule_span(rule)]);
        if with_hint {
            line.spans.extend([Self::blank(GAP), self.hint_span()]);
        }
        at
    }

    /// The hint's row of its own, `row` cells wide, from the text column: the
    /// rule, a gap and the hint, or the hint alone (cut with `…`) when `row`
    /// has no room for a rule. And the index of the rule's span, if any.
    fn own_row(&self, row: usize) -> (Line<'static>, Option<usize>) {
        match own_row_rule(row, self.width()) {
            Some(rule) => {
                let spans = vec![self.rule_span(rule), Self::blank(GAP), self.hint_span()];
                (Line::from(spans), Some(0))
            }
            None => (ellipsize(Line::from(self.hint_span()), row, false), None),
        }
    }
}

/// Adds the rule and the hint to the description `rows` of a subagent call,
/// whose text area is `text_width` cells wide. The title row is the last of
/// `rows`; it is `text_width + PADDING_REACH` cells wide after its lead, so
/// that it reaches two cells into the block's padding.
///
/// A folded row that has no room for the hint shows a rule alone, or nothing.
/// An unfolded row always shows the hint: when it does not fit on the title
/// row it gets a row of its own below the description, and the description's
/// rows have no rule. Returns where the chrome is, before the lead.
pub(super) fn with_open_hint(
    rows: &mut Vec<Line<'static>>,
    hint: &OpenHint<'_>,
    text_width: usize,
    folded: bool,
) -> Option<TitleChrome> {
    let row = text_width + usize::from(PADDING_REACH);
    let last = rows.len().checked_sub(1)?;
    let text = rows[last].width();
    // `rule` is the index of the rule's span; the gap before it starts the
    // chrome.
    let chrome = |rule: usize| TitleChrome {
        row: last,
        span: rule - 1,
        own_row: false,
        rule: Some(rule),
    };
    match Trailer::fit(text, row, hint.width()) {
        Trailer::Full { rule } => Some(chrome(hint.extend(&mut rows[last], rule, true))),
        Trailer::Rule { rule } if folded => Some(chrome(hint.extend(&mut rows[last], rule, false))),
        _ if folded => None,
        _ => {
            let (own, rule) = hint.own_row(row);
            rows.push(own);
            Some(TitleChrome {
                row: last + 1,
                span: 0,
                own_row: true,
                rule,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color::{Blue, Red};

    fn base() -> Style {
        Style::new().fg(Red)
    }

    fn muted() -> Style {
        Style::new().fg(Blue)
    }

    fn texts(rows: &[Line<'_>]) -> Vec<String> {
        rows.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_duration_follows_the_last_row_when_it_fits() {
        let rows = wrapped("Do it", Some(" · 1.2s"), 20, base(), muted());
        assert_eq!(texts(&rows), ["Do it · 1.2s"]);
        let styles: Vec<_> = rows[0].spans.iter().map(|s| s.style.fg).collect();
        assert_eq!(styles, [Some(Red), Some(Blue)]);
    }

    #[test]
    fn the_duration_goes_to_a_row_of_its_own_rather_than_be_split() {
        // `Do it now ·` would fit, `5m 2s` would not.
        let rows = wrapped("Do it now", Some(" · 5m 2s"), 12, base(), muted());
        assert_eq!(texts(&rows), ["Do it now", "· 5m 2s"]);
        assert_eq!(rows[1].spans[0].style.fg, Some(Blue));
        // The full width is still usable.
        let rows = wrapped("Do it now", Some(" · 5m 2s"), 17, base(), muted());
        assert_eq!(texts(&rows), ["Do it now · 5m 2s"]);
    }

    #[test]
    fn a_wrapped_description_keeps_its_style_and_the_duration_is_muted() {
        let rows = wrapped("aaa bbb ccc ddd", Some(" · 340ms"), 8, base(), muted());
        assert_eq!(texts(&rows), ["aaa bbb", "ccc ddd", "· 340ms"]);
        assert!(rows[..2].iter().all(|r| r.spans[0].style.fg == Some(Red)));
        assert_eq!(rows[2].spans[0].style.fg, Some(Blue));
    }

    #[test]
    fn a_duration_wider_than_a_row_is_broken() {
        let rows = wrapped("Do", Some(" · 1.2s"), 4, base(), muted());
        assert_eq!(texts(&rows), ["Do", "·", "1.2s"]);
        assert!(rows[1..].iter().all(|r| r.spans[0].style.fg == Some(Blue)));
    }

    #[test]
    fn no_duration_wraps_as_before() {
        let rows = wrapped("aaa bbb ccc", None, 8, base(), muted());
        assert_eq!(texts(&rows), ["aaa bbb", "ccc"]);
    }

    #[test]
    fn the_folded_row_cuts_the_description_and_keeps_the_tail() {
        let tail = Tail {
            agent: Some(" (agent)"),
            duration: Some(" · 5m 2s"),
        };
        let row = folded("A long description", tail, 30, base(), muted());
        assert_eq!(row.to_string(), "A long descri… (agent) · 5m 2s");
        let fgs: Vec<_> = row.spans.iter().map(|s| s.style.fg).collect();
        assert_eq!(fgs, [Some(Red), Some(Red), Some(Red), Some(Blue)]);
    }

    #[test]
    fn a_tail_without_room_for_the_description_cuts_the_whole_row() {
        let tail = Tail {
            agent: None,
            duration: Some(" · 5m 2s"),
        };
        let row = folded("A long description", tail, 8, base(), muted());
        assert_eq!(row.to_string(), "A long …");
        // The `…` replaces description text.
        assert_eq!(row.spans.last().unwrap().style.fg, Some(Red));
    }

    // ---- The rule and the open hint ----

    const HINT: &str = "a to open transcript";
    const H: usize = HINT.len();

    fn rule_style() -> Style {
        Style::new().fg(ratatui::style::Color::Green)
    }

    fn open_hint() -> OpenHint<'static> {
        OpenHint {
            text: HINT,
            rule: rule_style(),
            hint: muted(),
        }
    }

    fn description(text: &str) -> Vec<Line<'static>> {
        vec![Line::from(Span::styled(text.to_owned(), base()))]
    }

    /// The only row after adding the hint to a description `d` cells wide in
    /// a text area `t` cells wide.
    fn one_row(d: usize, t: usize, folded: bool) -> (Vec<Line<'static>>, Option<TitleChrome>) {
        let mut rows = description(&"x".repeat(d));
        let chrome = with_open_hint(&mut rows, &open_hint(), t, folded);
        (rows, chrome)
    }

    #[test]
    fn the_trailer_shrinks_the_rule_then_drops_the_hint_then_the_rule() {
        let (d, h) = (10, H);
        let table = [
            // r = d + 7 + h: the shortest rule with the hint.
            (d + 7 + h + 5, Trailer::Full { rule: 8 }),
            (d + 7 + h, Trailer::Full { rule: 3 }),
            // One cell narrower: no hint, the rule runs to the end.
            (d + 6 + h, Trailer::Rule { rule: h + 4 }),
            (d + 5 + 1, Trailer::Rule { rule: 4 }),
            (d + 5, Trailer::Rule { rule: 3 }),
            // Below d + 5 the row is as it was.
            (d + 4, Trailer::Nothing),
            (d, Trailer::Nothing),
            (0, Trailer::Nothing),
        ];
        for (row, want) in table {
            assert_eq!(Trailer::fit(d, row, h), want, "r = {row}");
        }
    }

    #[test]
    fn a_title_row_with_room_is_text_gap_rule_gap_hint_and_r_cells_wide() {
        let t = 60;
        let (rows, chrome) = one_row(22, t, true);
        let row = rows[0].to_string();
        // r = t + 2 = 62: 22 + 2 + rule + 2 + 20.
        let rule = 62 - 22 - 4 - H;
        assert_eq!(
            row,
            format!("{}  {}  {HINT}", "x".repeat(22), "─".repeat(rule))
        );
        assert_eq!(rows[0].width(), t + 2);
        assert_eq!(
            chrome,
            Some(TitleChrome {
                row: 0,
                span: 1,
                own_row: false,
                rule: Some(2),
            })
        );
        let styles: Vec<_> = rows[0].spans.iter().map(|s| s.style).collect();
        assert_eq!(styles[0], base());
        assert_eq!(styles[2], rule_style());
        assert_eq!(styles[4], muted());
        assert!(styles.iter().all(|s| s.bg.is_none()));
    }

    #[test]
    fn at_the_narrowest_rule_with_the_hint_the_rule_is_3_cells() {
        let d = 12;
        let t = d + 7 + H - 2;
        let (rows, _) = one_row(d, t, true);
        assert_eq!(
            rows[0].to_string(),
            format!("{}  ───  {HINT}", "x".repeat(d))
        );
        assert_eq!(rows[0].width(), t + 2);
    }

    #[test]
    fn one_cell_narrower_the_hint_goes_and_the_rule_runs_to_the_end() {
        let d = 12;
        let t = d + 6 + H - 2;
        let (rows, chrome) = one_row(d, t, true);
        let rule = t + 2 - d - 2;
        assert_eq!(
            rows[0].to_string(),
            format!("{}  {}", "x".repeat(d), "─".repeat(rule))
        );
        assert_eq!(rows[0].width(), t + 2);
        assert_eq!(chrome.unwrap().rule, Some(2));
    }

    #[test]
    fn at_d_plus_5_the_rule_is_3_cells_and_at_d_plus_4_the_row_is_unchanged() {
        let d = 12;
        let (rows, chrome) = one_row(d, d + 5 - 2, true);
        assert_eq!(rows[0].to_string(), format!("{}  ───", "x".repeat(d)));
        assert!(chrome.is_some());
        let (rows, chrome) = one_row(d, d + 4 - 2, true);
        assert_eq!(rows[0].to_string(), "x".repeat(d));
        assert_eq!(chrome, None);
    }

    #[test]
    fn a_description_that_fills_the_text_area_has_neither_rule_nor_hint() {
        let (rows, chrome) = one_row(30, 30, true);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].width(), 30);
        assert_eq!(chrome, None);
    }

    #[test]
    fn a_cut_folded_row_gets_neither_rule_nor_hint() {
        let tail = Tail {
            agent: Some(" (agent)"),
            duration: Some(" · 5m 2s"),
        };
        for t in [20, 30, 40] {
            let mut rows = vec![folded(&"d".repeat(60), tail, t, base(), muted())];
            let before = rows[0].to_string();
            let chrome = with_open_hint(&mut rows, &open_hint(), t, true);
            assert_eq!(chrome, None, "t = {t}");
            assert_eq!(rows[0].to_string(), before);
        }
    }

    #[test]
    fn widths_are_display_cells_not_characters() {
        // 3 wide characters: 6 cells.
        let mut rows = description("日本語");
        let t = 6 + 7 + H - 2;
        let chrome = with_open_hint(&mut rows, &open_hint(), t, true).unwrap();
        assert_eq!(chrome.span, 1);
        assert_eq!(rows[0].width(), t + 2);
        assert_eq!(rows[0].to_string(), "日本語  ───  a to open transcript");
        // A cut ending in a wide character is a cell short and gets nothing.
        let cut = folded(
            "日本語日本語日本語",
            Tail {
                agent: None,
                duration: None,
            },
            8,
            base(),
            muted(),
        );
        assert_eq!(cut.width(), 7);
        let mut rows = vec![cut];
        assert_eq!(with_open_hint(&mut rows, &open_hint(), 8, true), None);
    }

    #[test]
    fn an_unfolded_hint_without_room_goes_on_a_row_of_its_own() {
        let t = 40;
        let mut rows = description(&"x".repeat(38));
        let chrome = with_open_hint(&mut rows, &open_hint(), t, false).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].to_string(), "x".repeat(38), "no rule on the text");
        let rule = t + 2 - 2 - H;
        assert_eq!(rows[1].to_string(), format!("{}  {HINT}", "─".repeat(rule)));
        assert_eq!(rows[1].width(), t + 2);
        assert_eq!(
            chrome,
            TitleChrome {
                row: 1,
                span: 0,
                own_row: true,
                rule: Some(0)
            }
        );
    }

    #[test]
    fn a_folded_hint_without_room_is_shown_nowhere() {
        let mut rows = description(&"x".repeat(38));
        with_open_hint(&mut rows, &open_hint(), 40, true);
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].to_string().contains("transcript"));
    }

    #[test]
    fn the_hints_own_row_has_a_rule_of_3_cells_at_r_equal_h_plus_5() {
        let t = H + 5 - 2;
        let mut rows = description(&"x".repeat(t));
        with_open_hint(&mut rows, &open_hint(), t, false);
        assert_eq!(rows[1].to_string(), format!("───  {HINT}"));
        assert_eq!(rows[1].width(), t + 2);
    }

    #[test]
    fn below_h_plus_5_the_hints_own_row_is_the_hint_alone() {
        let t = H + 4 - 2;
        let mut rows = description(&"x".repeat(t));
        let chrome = with_open_hint(&mut rows, &open_hint(), t, false).unwrap();
        assert_eq!(rows[1].to_string(), HINT);
        assert_eq!(chrome.rule, None);
        assert!(chrome.own_row);
    }

    #[test]
    fn a_hint_wider_than_the_row_is_cut_with_an_ellipsis() {
        let t = 8;
        let mut rows = description(&"x".repeat(t));
        with_open_hint(&mut rows, &open_hint(), t, false);
        assert_eq!(rows[1].to_string(), "a to open…");
        assert_eq!(rows[1].width(), t + 2);
    }
}
