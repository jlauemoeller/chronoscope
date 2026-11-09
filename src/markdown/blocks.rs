//! Parsing markdown into a block tree with styled inline text.

use std::iter::Peekable;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::Style;

use super::Styles;
use super::prose::Inline;
use crate::theme::emphasis;

/// A block of a document.
#[derive(Debug)]
pub(super) enum Block {
    /// A paragraph or heading. `loose` is set for a paragraph the source
    /// wrapped in its own block, which makes the list around it loose.
    Paragraph {
        inline: Inline,
        loose: bool,
    },
    Code {
        lang: Option<String>,
        text: String,
    },
    Rule,
    Quote(Vec<Block>),
    List(List),
    Table(Table),
}

#[derive(Debug)]
pub(super) struct List {
    /// The number of the first item of an ordered list.
    pub start: Option<u64>,
    pub items: Vec<Vec<Block>>,
    /// Whether items sit on consecutive lines, with no blank line.
    pub tight: bool,
}

#[derive(Debug)]
pub(super) struct Table {
    pub aligns: Vec<Alignment>,
    pub header: Vec<Inline>,
    pub rows: Vec<Vec<Inline>>,
}

/// Parses `src` into blocks.
pub(super) fn parse(src: &str, styles: &Styles) -> Vec<Block> {
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    let mut reader = Reader {
        events: Parser::new_ext(src, options).peekable(),
        styles: *styles,
    };
    reader.blocks()
}

struct Reader<'a, 's> {
    events: Peekable<Parser<'a>>,
    styles: Styles<'s>,
}

/// Whether `tag` styles a span of text rather than starting a block.
fn is_inline_tag(tag: &Tag) -> bool {
    matches!(
        tag,
        Tag::Emphasis
            | Tag::Strong
            | Tag::Strikethrough
            | Tag::Superscript
            | Tag::Subscript
            | Tag::Link { .. }
            | Tag::Image { .. }
    )
}

fn heading_number(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// The style inside `tag`, derived from the `outer` style.
fn inline_style(tag: &Tag, outer: Style) -> Style {
    match tag {
        Tag::Emphasis => emphasis::emphasis(outer),
        Tag::Strong => emphasis::strong(outer),
        Tag::Strikethrough => emphasis::strikethrough(outer),
        Tag::Link { .. } => emphasis::link(outer),
        _ => outer,
    }
}

impl Reader<'_, '_> {
    /// Reads blocks up to and including the end of the enclosing container
    /// (or the end of the document).
    fn blocks(&mut self) -> Vec<Block> {
        let mut out = Vec::new();
        while let Some(event) = self.events.peek().cloned() {
            match event {
                Event::End(_) => {
                    self.events.next();
                    break;
                }
                Event::Rule => {
                    self.events.next();
                    out.push(Block::Rule);
                }
                Event::Start(tag) if !is_inline_tag(&tag) => {
                    self.events.next();
                    out.extend(self.block(tag));
                }
                _ => out.push(self.loose_text()),
            }
        }
        out
    }

    /// Text that sits directly in a tight list item, as a paragraph.
    fn loose_text(&mut self) -> Block {
        let inline = self.inline(self.styles.base, false);
        Block::Paragraph {
            inline,
            loose: false,
        }
    }

    fn block(&mut self, tag: Tag) -> Vec<Block> {
        match tag {
            Tag::Paragraph => {
                let inline = self.inline(self.styles.base, true);
                vec![Block::Paragraph {
                    inline,
                    loose: true,
                }]
            }
            Tag::Heading { level, .. } => {
                let style = self.styles.heading(heading_number(level));
                let inline = self.inline(style, true);
                vec![Block::Paragraph {
                    inline,
                    loose: false,
                }]
            }
            Tag::BlockQuote(_) => vec![Block::Quote(self.blocks())],
            Tag::CodeBlock(kind) => vec![self.code_block(&kind)],
            Tag::HtmlBlock => vec![self.html_block()],
            Tag::List(start) => vec![Block::List(self.list(start))],
            Tag::Table(aligns) => vec![Block::Table(self.table(aligns))],
            _ => self.blocks(),
        }
    }

    /// Reads inline events into styled text, starting from `style`.
    ///
    /// With `explicit` the end event of the enclosing block is consumed;
    /// otherwise reading stops in front of it, or in front of a block start.
    fn inline(&mut self, style: Style, explicit: bool) -> Inline {
        let mut out = Inline::default();
        let mut stack = vec![style];
        while let Some(event) = self.events.peek().cloned() {
            let current = stack.last().copied().unwrap_or(style);
            match event {
                Event::End(_) if stack.len() == 1 => {
                    if explicit {
                        self.events.next();
                    }
                    break;
                }
                Event::Rule => break,
                Event::Start(ref tag) if !is_inline_tag(tag) => break,
                Event::Start(ref tag) => stack.push(inline_style(tag, current)),
                Event::End(_) => {
                    stack.pop();
                }
                other => self.push_text(&mut out, other, current),
            }
            self.events.next();
        }
        out
    }

    fn push_text(&self, out: &mut Inline, event: Event, style: Style) {
        match event {
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                out.push(&text, style);
            }
            Event::Code(code) => out.push(&code, style.patch(self.styles.command)),
            Event::SoftBreak => out.push(" ", style),
            Event::HardBreak => out.push("\n", style),
            _ => {}
        }
    }

    /// Reads the text of a code block, up to its end.
    fn text_until_end(&mut self) -> String {
        let mut text = String::new();
        for event in self.events.by_ref() {
            match event {
                Event::Text(t) | Event::Html(t) => text.push_str(&t),
                Event::End(_) => break,
                _ => {}
            }
        }
        text
    }

    fn code_block(&mut self, kind: &CodeBlockKind) -> Block {
        let lang = match kind {
            CodeBlockKind::Fenced(info) => info
                .split(|c: char| c.is_whitespace() || c == ',' || c == '{')
                .next()
                .filter(|lang| !lang.is_empty())
                .map(str::to_string),
            CodeBlockKind::Indented => None,
        };
        Block::Code {
            lang,
            text: self.text_until_end(),
        }
    }

    fn html_block(&mut self) -> Block {
        let text = self.text_until_end();
        let mut inline = Inline::default();
        inline.push(text.trim_end_matches('\n'), self.styles.base);
        Block::Paragraph {
            inline,
            loose: false,
        }
    }

    fn list(&mut self, start: Option<u64>) -> List {
        let mut items = Vec::new();
        while let Some(event) = self.events.next() {
            match event {
                Event::Start(Tag::Item) => items.push(self.blocks()),
                Event::End(TagEnd::List(_)) => break,
                _ => {}
            }
        }
        let tight = !items
            .iter()
            .flatten()
            .any(|block| matches!(block, Block::Paragraph { loose: true, .. }));
        List {
            start,
            items,
            tight,
        }
    }

    fn table(&mut self, aligns: Vec<Alignment>) -> Table {
        let header_style = emphasis::table_header(self.styles.base);
        let mut table = Table {
            aligns,
            header: Vec::new(),
            rows: Vec::new(),
        };
        let mut cells: Vec<Inline> = Vec::new();
        let mut in_header = false;
        while let Some(event) = self.events.next() {
            match event {
                Event::Start(Tag::TableHead) => in_header = true,
                Event::Start(Tag::TableCell) => {
                    let style = if in_header {
                        header_style
                    } else {
                        self.styles.base
                    };
                    cells.push(self.inline(style, true).single_line());
                }
                Event::End(TagEnd::TableHead) => {
                    in_header = false;
                    table.header = std::mem::take(&mut cells);
                }
                Event::End(TagEnd::TableRow) => table.rows.push(std::mem::take(&mut cells)),
                Event::End(TagEnd::Table) => break,
                _ => {}
            }
        }
        table
    }
}
