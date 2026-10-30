//! Syntax highlighting of code, mapped to the theme's `syntax.*` tokens.
//!
//! syntect only parses: it assigns scopes to text. The scopes are mapped to
//! tokens here instead of going through a syntect colour theme, so a theme
//! change needs no work and no colour is named in this module.

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

use ratatui::style::Style;
use ratatui::text::{Line, Span};
use syntect::parsing::{ParseState, Scope, ScopeStack, SyntaxReference, SyntaxSet};

use crate::theme::{Theme, Token};

static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
static LOADS: AtomicUsize = AtomicUsize::new(0);

/// The scope prefixes and the token each one maps to. For a piece of text the
/// innermost scope with a match wins.
const RULES: [(&str, Token); 9] = [
    ("keyword", Token::SyntaxKeyword),
    ("storage", Token::SyntaxKeyword),
    ("entity.name.type", Token::SyntaxType),
    ("support.type", Token::SyntaxType),
    ("entity.name.function", Token::SyntaxFunction),
    ("support.function", Token::SyntaxFunction),
    ("string", Token::SyntaxString),
    ("constant.numeric", Token::SyntaxNumber),
    ("constant.language", Token::SyntaxNumber),
];
const COMMENT: &str = "comment";

/// The syntax definitions, loaded on first use and shared afterwards.
pub(super) fn syntax_set() -> &'static SyntaxSet {
    SYNTAXES.get_or_init(|| {
        LOADS.fetch_add(1, Ordering::SeqCst);
        SyntaxSet::load_defaults_newlines()
    })
}

/// How many times the syntax set has been loaded.
#[cfg(test)]
pub(super) fn syntax_set_loads() -> usize {
    LOADS.load(Ordering::SeqCst)
}

/// The scope rules, parsed once.
fn rules() -> &'static [(Scope, Token)] {
    static PARSED: OnceLock<Vec<(Scope, Token)>> = OnceLock::new();
    PARSED.get_or_init(|| {
        RULES
            .iter()
            .chain(std::iter::once(&(COMMENT, Token::SyntaxComment)))
            .filter_map(|(name, token)| Scope::new(name).ok().map(|scope| (scope, *token)))
            .collect()
    })
}

/// The token for the text under `stack`: the innermost scope that a rule
/// matches, or `syntax.plain`.
pub(super) fn token_for(stack: &ScopeStack) -> Token {
    stack
        .as_slice()
        .iter()
        .rev()
        .find_map(|scope| {
            rules()
                .iter()
                .find(|(rule, _)| rule.is_prefix_of(*scope))
                .map(|(_, token)| *token)
        })
        .unwrap_or(Token::SyntaxPlain)
}

fn syntax_for(lang: &str) -> Option<&'static SyntaxReference> {
    let token = lang.trim();
    if token.is_empty() {
        return None;
    }
    syntax_set().find_syntax_by_token(token)
}

fn push_span(spans: &mut Vec<Span<'static>>, text: &str, style: Style) {
    let text = text.trim_end_matches(['\n', '\r']);
    if text.is_empty() {
        return;
    }
    match spans.last_mut() {
        Some(last) if last.style == style => last.content.to_mut().push_str(text),
        _ => spans.push(Span::styled(text.to_string(), style)),
    }
}

/// One line (with its newline) as styled spans, advancing the parse state.
fn highlight_line(
    raw: &str,
    parser: &mut ParseState,
    stack: &mut ScopeStack,
    theme: &Theme,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let Ok(ops) = parser.parse_line(raw, syntax_set()) else {
        push_span(&mut spans, raw, theme.style(Token::SyntaxPlain));
        return spans;
    };
    let mut last = 0;
    for (at, op) in ops {
        push_span(&mut spans, &raw[last..at], theme.style(token_for(stack)));
        // An op that does not apply leaves the stack as it was.
        let _ = stack.apply(&op);
        last = at;
    }
    push_span(&mut spans, &raw[last..], theme.style(token_for(stack)));
    spans
}

/// Highlights `code` as the language or file extension `lang_or_extension`
/// (`rust`, `rs`, `py`, ...), one [`Line`] per source line, without wrapping.
///
/// Text is styled with the `syntax.*` tokens of `theme`. An unknown or empty
/// language gives `syntax.plain` throughout. A trailing newline does not add a
/// line. Tabs become four spaces.
pub fn highlight_code(code: &str, lang_or_extension: &str, theme: &Theme) -> Vec<Line<'static>> {
    let code = code.replace('\t', "    ");
    let plain = theme.style(Token::SyntaxPlain);
    let Some(syntax) = syntax_for(lang_or_extension) else {
        return code
            .lines()
            .map(|line| Line::from(plain_spans(line, plain)))
            .collect();
    };
    let mut parser = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    code.split_inclusive('\n')
        .map(|raw| Line::from(highlight_line(raw, &mut parser, &mut stack, theme)))
        .collect()
}

fn plain_spans(line: &str, style: Style) -> Vec<Span<'static>> {
    if line.is_empty() {
        Vec::new()
    } else {
        vec![Span::styled(line.to_string(), style)]
    }
}
