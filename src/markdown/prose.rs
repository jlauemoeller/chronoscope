//! Styled inline text and its word wrapping.

use ratatui::style::Style;
use ratatui::text::Span;
use textwrap::core::Fragment;
use textwrap::wrap_algorithms::wrap_first_fit;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// A run of text in one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Seg {
    pub text: String,
    pub style: Style,
}

/// Styled inline text. A `'\n'` in it is a hard line break.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Inline(pub Vec<Seg>);

impl Inline {
    /// Appends `text`, merging it into the last run when the style matches.
    pub fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        // A tab cannot be drawn: it would vanish and join its neighbours.
        let text = text.replace('\t', "    ");
        let text = text.as_str();
        match self.0.last_mut() {
            Some(last) if last.style == style => last.text.push_str(text),
            _ => self.0.push(Seg {
                text: text.to_string(),
                style,
            }),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.0.iter().all(|seg| seg.text.is_empty())
    }

    /// The text split at hard line breaks.
    pub fn split_lines(&self) -> Vec<Inline> {
        let mut lines = vec![Inline::default()];
        for seg in &self.0 {
            for (i, part) in seg.text.split('\n').enumerate() {
                if i > 0 {
                    lines.push(Inline::default());
                }
                if let Some(line) = lines.last_mut() {
                    line.push(part, seg.style);
                }
            }
        }
        lines
    }

    /// The text with every hard break replaced by a space.
    pub fn single_line(&self) -> Inline {
        let mut line = Inline::default();
        for seg in &self.0 {
            line.push(&seg.text.replace('\n', " "), seg.style);
        }
        line
    }

    /// The display width of the text in cells.
    pub fn width(&self) -> usize {
        self.0.iter().map(|seg| seg.text.width()).sum()
    }
}

/// A word and the whitespace that follows it, as textwrap sees it.
#[derive(Debug)]
struct Word {
    body: Vec<Seg>,
    gap: Vec<Seg>,
}

fn segs_width(segs: &[Seg]) -> usize {
    segs.iter().map(|seg| seg.text.width()).sum()
}

impl Fragment for Word {
    fn width(&self) -> f64 {
        segs_width(&self.body) as f64
    }

    fn whitespace_width(&self) -> f64 {
        segs_width(&self.gap) as f64
    }

    fn penalty_width(&self) -> f64 {
        0.0
    }
}

fn is_gap(c: char) -> bool {
    c == ' ' || c == '\t'
}

fn push_seg(segs: &mut Vec<Seg>, text: &str, style: Style) {
    match segs.last_mut() {
        Some(last) if last.style == style => last.text.push_str(text),
        _ => segs.push(Seg {
            text: text.to_string(),
            style,
        }),
    }
}

/// Splits `text` into words at spaces. Leading whitespace is dropped.
fn words(text: &Inline) -> Vec<Word> {
    let mut out: Vec<Word> = Vec::new();
    let mut current = Word {
        body: Vec::new(),
        gap: Vec::new(),
    };
    let mut in_gap = false;
    for seg in &text.0 {
        for c in seg.text.chars() {
            let mut buf = [0; 4];
            let s: &str = c.encode_utf8(&mut buf);
            if is_gap(c) {
                if !current.body.is_empty() {
                    in_gap = true;
                    push_seg(&mut current.gap, s, seg.style);
                }
            } else {
                if in_gap {
                    out.push(std::mem::replace(
                        &mut current,
                        Word {
                            body: Vec::new(),
                            gap: Vec::new(),
                        },
                    ));
                    in_gap = false;
                }
                push_seg(&mut current.body, s, seg.style);
            }
        }
    }
    if !current.body.is_empty() {
        out.push(current);
    }
    out
}

/// Splits a word wider than `width` into pieces of at most `width` cells,
/// never inside a grapheme. Only the last piece keeps the trailing gap.
fn break_word(word: Word, width: usize) -> Vec<Word> {
    if segs_width(&word.body) <= width {
        return vec![word];
    }
    let mut chunks: Vec<Vec<Seg>> = vec![Vec::new()];
    let mut used = 0;
    for seg in &word.body {
        for g in seg.text.graphemes(true) {
            let w = g.width();
            if used + w > width && used > 0 {
                chunks.push(Vec::new());
                used = 0;
            }
            if let Some(chunk) = chunks.last_mut() {
                push_seg(chunk, g, seg.style);
            }
            used += w;
        }
    }
    let last = chunks.len() - 1;
    let mut gap = Some(word.gap);
    chunks
        .into_iter()
        .enumerate()
        .map(|(i, body)| Word {
            body,
            gap: if i == last {
                gap.take().unwrap_or_default()
            } else {
                Vec::new()
            },
        })
        .collect()
}

fn line_spans(line: &[Word]) -> Vec<Span<'static>> {
    let mut segs: Vec<Seg> = Vec::new();
    for (i, word) in line.iter().enumerate() {
        for seg in &word.body {
            push_seg(&mut segs, &seg.text, seg.style);
        }
        if i + 1 < line.len() {
            for seg in &word.gap {
                push_seg(&mut segs, &seg.text, seg.style);
            }
        }
    }
    segs.into_iter()
        .map(|seg| Span::styled(seg.text, seg.style))
        .collect()
}

/// Word-wraps `text` (which has no hard breaks) to `width` cells.
///
/// Breaks fall at spaces. A word wider than `width` is broken between
/// graphemes. Always returns at least one (possibly empty) line.
pub(super) fn wrap(text: &Inline, width: usize) -> Vec<Vec<Span<'static>>> {
    let width = width.max(1);
    let words: Vec<Word> = words(text)
        .into_iter()
        .flat_map(|word| break_word(word, width))
        .collect();
    wrap_first_fit(&words, &[width as f64])
        .into_iter()
        .map(line_spans)
        .collect()
}
