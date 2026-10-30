use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

use super::*;
use crate::theme::{Role, Theme, Token, TokenOverride, builtin};

pub(super) const SAMPLE: &str = "\
# Release notes

## Highlights

A paragraph with *emphasis*, **strong text**, `inline_code` and a [link to the docs](https://example.com/docs) that is long enough to wrap onto more lines.

- First bullet
  - Nested bullet one
  - Nested bullet two with enough words to wrap around
- Second bullet

1. Ordered one
2. Ordered two

> A quoted remark that is long enough to wrap when the terminal is narrow.

| Name | Description | Status |
|---|---|---|
| parser | Turns markdown into blocks | done |
| renderer | Lays the blocks out as wrapped lines | in progress |

```rust
// Say hello.
fn main() {
    let answer = 42;
    println!(\"The answer is {answer}, printed from main with a long line\");
}
```

---
";

fn text(lines: &[Line]) -> Vec<String> {
    lines.iter().map(|l| l.to_string()).collect()
}

/// A theme whose seven syntax tokens (and the inline-code token) all have
/// different foregrounds, derived from base roles.
fn distinct_theme() -> Theme {
    let theme = Theme::default();
    let fg = |role| TokenOverride::fg(theme.role(role));
    theme
        .clone()
        .with_token(Token::Command, fg(Role::Warning))
        .with_token(Token::SyntaxKeyword, fg(Role::Accent))
        .with_token(Token::SyntaxType, fg(Role::Secondary))
        .with_token(Token::SyntaxFunction, fg(Role::Primary))
        .with_token(Token::SyntaxString, fg(Role::Warning))
        .with_token(Token::SyntaxNumber, fg(Role::Destructive))
        .with_token(Token::SyntaxComment, fg(Role::MutedForeground))
        .with_token(Token::SyntaxPlain, fg(Role::Foreground))
}

fn base_of(theme: &Theme) -> Style {
    Style::new().fg(theme.role(Role::Foreground))
}

fn render(src: &str, width: u16) -> Vec<Line<'static>> {
    let theme = Theme::default();
    render_markdown(src, width, base_of(&theme), &theme)
}

/// The text and style of each span of a line.
fn runs(line: &Line) -> Vec<(String, Style)> {
    line.spans
        .iter()
        .map(|s| (s.content.to_string(), s.style))
        .collect()
}

fn run(text: &str, style: Style) -> (String, Style) {
    (text.to_string(), style)
}

fn muted(theme: &Theme) -> Style {
    Style::new().fg(theme.role(Role::MutedForeground))
}

fn width_of(line: &Line) -> usize {
    unicode_width::UnicodeWidthStr::width(line.to_string().as_str())
}

#[test]
fn headings_of_levels_one_to_three() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let lines = render("# One\n\n## Two\n\n### Three\n\nbody", 40);
    assert_eq!(text(&lines), ["One", "", "Two", "", "Three", "", "body"]);
    let bold = base.add_modifier(Modifier::BOLD);
    assert_eq!(
        runs(&lines[0]),
        [run("One", bold.add_modifier(Modifier::UNDERLINED))]
    );
    assert_eq!(runs(&lines[2]), [run("Two", bold)]);
    assert_eq!(runs(&lines[4]), [run("Three", bold)]);
    assert_eq!(runs(&lines[6]), [run("body", base)]);
}

#[test]
fn heading_marks_are_not_shown_and_a_heading_wraps() {
    let lines = render("## A heading that is long enough to wrap", 20);
    assert_eq!(text(&lines), ["A heading that is", "long enough to wrap"]);
}

#[test]
fn emphasis_strong_and_strike() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let lines = render("a *b* **c** ~~d~~ e ***f***", 40);
    assert_eq!(text(&lines), ["a b c d e f"]);
    assert_eq!(
        runs(&lines[0]),
        [
            run("a ", base),
            run("b", base.add_modifier(Modifier::ITALIC)),
            run(" ", base),
            run("c", base.add_modifier(Modifier::BOLD)),
            run(" ", base),
            run("d", base.add_modifier(Modifier::CROSSED_OUT)),
            run(" e ", base),
            run("f", base.add_modifier(Modifier::ITALIC | Modifier::BOLD)),
        ]
    );
}

#[test]
fn inline_code_uses_the_command_token_and_shows_no_backticks() {
    let theme = distinct_theme();
    let base = base_of(&theme);
    let lines = render_markdown("run `cargo test` now", 40, base, &theme);
    assert_eq!(text(&lines), ["run cargo test now"]);
    let code = base.patch(theme.style(Token::Command));
    assert_ne!(code, base);
    assert_eq!(
        runs(&lines[0]),
        [
            run("run ", base),
            run("cargo test", code),
            run(" now", base)
        ]
    );
}

#[test]
fn inline_code_keeps_the_surrounding_emphasis() {
    let theme = distinct_theme();
    let base = base_of(&theme);
    let lines = render_markdown("**a `b`**", 40, base, &theme);
    let bold = base.add_modifier(Modifier::BOLD);
    assert_eq!(
        runs(&lines[0]),
        [
            run("a ", bold),
            run("b", bold.patch(theme.style(Token::Command)))
        ]
    );
}

#[test]
fn link_text_is_underlined_and_the_url_is_not_shown() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let lines = render("see [the docs](https://example.com/x) now", 60);
    assert_eq!(text(&lines), ["see the docs now"]);
    assert_eq!(
        runs(&lines[0]),
        [
            run("see ", base),
            run("the docs", base.add_modifier(Modifier::UNDERLINED)),
            run(" now", base),
        ]
    );
}

#[test]
fn nested_bullet_lists_wrap_with_a_hanging_indent() {
    let src = "- alpha beta gamma delta\n  - nested one two three\n  - second nested\n- last";
    let lines = render(src, 16);
    assert_eq!(
        text(&lines),
        [
            "• alpha beta",
            "  gamma delta",
            "  • nested one",
            "    two three",
            "  • second",
            "    nested",
            "• last",
        ]
    );
}

#[test]
fn ordered_lists_wrap_under_the_item_text() {
    let lines = render(
        "1. first item with words\n2. second\n3. third item here",
        14,
    );
    assert_eq!(
        text(&lines),
        [
            "1. first item",
            "   with words",
            "2. second",
            "3. third item",
            "   here",
        ]
    );
}

#[test]
fn ordered_markers_are_padded_to_the_widest() {
    let lines = render("9. nine\n10. ten", 20);
    assert_eq!(text(&lines), ["9.  nine", "10. ten"]);
}

#[test]
fn bullets_nest_inside_ordered_items_and_back() {
    let lines = render("1. one\n   - inner\n2. two", 20);
    assert_eq!(text(&lines), ["1. one", "  • inner", "2. two"]);
}

#[test]
fn nested_lists_are_indented_two_cells_per_level_under_any_marker() {
    let src = "9. nine\n   1. inner one with words\n   2. inner two\n10. ten\n    - deep";
    let lines = render(src, 16);
    assert_eq!(
        text(&lines),
        [
            "9.  nine",
            "  1. inner one",
            "     with words",
            "  2. inner two",
            "10. ten",
            "  • deep",
        ]
    );
}

#[test]
fn list_lines_have_a_marker_span_and_a_hang_indent_span() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let lines = render("1. alpha beta gamma\n2. two", 12);
    assert_eq!(text(&lines), ["1. alpha", "   beta", "   gamma", "2. two"]);
    assert_eq!(runs(&lines[0]), [run("1. ", base), run("alpha", base)]);
    assert_eq!(
        runs(&lines[1]),
        [run("   ", Style::new()), run("beta", base)]
    );
    let bullets = render("- alpha beta\n  - inner text", 9);
    assert_eq!(
        text(&bullets),
        ["• alpha", "  beta", "  • inner", "    text"]
    );
    assert_eq!(
        runs(&bullets[2]),
        [run("  ", Style::new()), run("• ", base), run("inner", base)]
    );
}

#[test]
fn a_loose_list_keeps_a_blank_line_between_items() {
    let lines = render("- one\n\n- two", 20);
    assert_eq!(text(&lines), ["• one", "", "• two"]);
}

#[test]
fn list_markers_use_the_base_style() {
    let theme = Theme::default();
    let lines = render("- item", 20);
    assert_eq!(
        runs(&lines[0]),
        [run("• ", base_of(&theme)), run("item", base_of(&theme))]
    );
}

#[test]
fn block_quote_prefixes_every_line_with_a_muted_bar() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let lines = render("> quoted *words* that wrap\n>\n> second", 16);
    assert_eq!(
        text(&lines),
        ["│ quoted words", "│ that wrap", "│ ", "│ second"]
    );
    assert_eq!(
        runs(&lines[0]),
        [
            run("│ ", muted(&theme)),
            run("quoted ", base),
            run("words", base.add_modifier(Modifier::ITALIC)),
        ]
    );
}

#[test]
fn a_table_has_aligned_columns_and_a_header_rule() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let src = "| Name | Qty |\n|---|---|\n| apple | 3 |\n| fig | 12 |";
    let lines = render(src, 40);
    assert_eq!(
        text(&lines),
        ["Name  │ Qty", "──────┼────", "apple │ 3", "fig   │ 12"]
    );
    let bold = base.add_modifier(Modifier::BOLD);
    assert_eq!(
        runs(&lines[0]),
        [
            run("Name", bold),
            run(" ", Style::new()),
            run(" │ ", muted(&theme)),
            run("Qty", bold),
        ]
    );
    assert_eq!(runs(&lines[1]), [run("──────┼────", muted(&theme))]);
}

#[test]
fn table_cells_honour_alignment_and_keep_inline_styles() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let src = "| L | R | C |\n|:--|--:|:-:|\n| `a` | **b** | c |";
    let lines = render(src, 40);
    assert_eq!(text(&lines)[2], "a │ b │ c");
    let widest = "| long left | long right | long mid |\n|:--|--:|:-:|\n| x | y | z |";
    let lines = render(widest, 60);
    assert_eq!(
        text(&lines),
        [
            "long left │ long right │ long mid",
            "──────────┼────────────┼─────────",
            "x         │          y │    z    ",
        ]
    );
    assert!(
        render(src, 40)[2]
            .spans
            .iter()
            .any(|s| s.content == "b" && s.style == base.add_modifier(Modifier::BOLD))
    );
}

#[test]
fn a_wide_table_truncates_its_widest_cells_with_an_ellipsis() {
    let src = "| Name | Description | Status |\n|---|---|---|\n\
               | parser | Turns markdown into blocks | done |\n\
               | renderer | Lays the blocks out as wrapped lines | in progress |";
    let lines = render(src, 40);
    assert_eq!(
        text(&lines),
        [
            "Name     │ Description     │ Status",
            "─────────┼─────────────────┼────────────",
            "parser   │ Turns markdown… │ done",
            "renderer │ Lays the block… │ in progress",
        ]
    );
    assert!(lines.iter().all(|l| width_of(l) <= 40));
}

#[test]
fn a_table_that_cannot_fit_shrinks_every_column_to_one_cell() {
    let lines = render("| ab | cd |\n|---|---|\n| ef | gh |", 4);
    assert_eq!(text(&lines), ["… │ …", "──┼──", "… │ …"]);
}

#[test]
fn horizontal_rule_spans_the_width_in_the_muted_colour() {
    let theme = Theme::default();
    let lines = render("above\n\n---\n\nbelow", 12);
    assert_eq!(text(&lines), ["above", "", "────────────", "", "below"]);
    assert_eq!(runs(&lines[2]), [run(&"─".repeat(12), muted(&theme))]);
}

#[test]
fn fenced_code_is_indented_highlighted_and_not_word_wrapped() {
    let theme = distinct_theme();
    let base = base_of(&theme);
    let src = "text\n\n```rust\nfn main() { 42 }\n```\n\nafter";
    let lines = render_markdown(src, 30, base, &theme);
    assert_eq!(
        text(&lines),
        ["text", "", "  fn main() { 42 }", "", "after"]
    );
    let kw = theme.style(Token::SyntaxKeyword);
    let func = theme.style(Token::SyntaxFunction);
    let plain = theme.style(Token::SyntaxPlain);
    let num = theme.style(Token::SyntaxNumber);
    assert_eq!(
        runs(&lines[2]),
        [
            run("  ", Style::new()),
            run("fn", kw),
            run(" ", plain),
            run("main", func),
            run("() { ", plain),
            run("42", num),
            run(" }", plain),
        ]
    );
}

#[test]
fn long_code_lines_are_char_wrapped_to_the_remaining_width() {
    let lines = render("```\nabcdefghijklmnopqrstuvwxyz\n\n  indented\n```", 12);
    assert_eq!(
        text(&lines),
        [
            "  abcdefghij",
            "  klmnopqrst",
            "  uvwxyz",
            "",
            "    indented"
        ]
    );
}

#[test]
fn an_unknown_code_language_uses_the_plain_token() {
    let theme = distinct_theme();
    let base = base_of(&theme);
    let lines = render_markdown("```nosuchlang\nfn x = \"a\" 1\n```", 40, base, &theme);
    assert_eq!(
        runs(&lines[0]),
        [
            run("  ", Style::new()),
            run("fn x = \"a\" 1", theme.style(Token::SyntaxPlain))
        ]
    );
}

#[test]
fn blocks_are_separated_by_one_blank_line_and_nothing_trails() {
    let lines = render("# T\n\npara\n\n- a\n\n---\n\n> q\n", 20);
    assert_eq!(
        text(&lines),
        [
            "T",
            "",
            "para",
            "",
            "• a",
            "",
            "─".repeat(20).as_str(),
            "",
            "│ q"
        ]
    );
    assert!(render("", 20).is_empty());
    assert!(render("\n\n", 20).is_empty());
    assert!(render("text", 0).is_empty());
}

#[test]
fn soft_breaks_join_lines_and_hard_breaks_split_them() {
    let lines = render("one\ntwo  \nthree\\\nfour", 40);
    assert_eq!(text(&lines), ["one two", "three", "four"]);
}

#[test]
fn prose_wraps_at_word_boundaries_within_width() {
    let lines = render("the quick brown fox jumps over the lazy dog", 15);
    assert_eq!(
        text(&lines),
        ["the quick brown", "fox jumps over", "the lazy dog"]
    );
}

#[test]
fn wrapping_keeps_the_style_of_each_word() {
    let theme = Theme::default();
    let base = base_of(&theme);
    let lines = render("aa **bb cc** dd", 5);
    let bold = base.add_modifier(Modifier::BOLD);
    assert_eq!(text(&lines), ["aa bb", "cc dd"]);
    assert_eq!(runs(&lines[0]), [run("aa ", base), run("bb", bold)]);
    assert_eq!(runs(&lines[1]), [run("cc", bold), run(" dd", base)]);
}

#[test]
fn a_word_longer_than_the_width_is_broken() {
    let lines = render("see /very/long/path/to/some/file.rs ok", 10);
    assert_eq!(
        text(&lines),
        ["see", "/very/long", "/path/to/s", "ome/file.r", "s ok"]
    );
}

#[test]
fn wide_characters_count_two_cells() {
    let lines = render("漢字漢字 漢字", 5);
    assert_eq!(text(&lines), ["漢字", "漢字", "漢字"]);
    let lines = render("漢字 漢字", 5);
    assert_eq!(text(&lines), ["漢字", "漢字"]);
    let lines = render("ab 漢字", 7);
    assert_eq!(text(&lines), ["ab 漢字"]);
    let lines = render("ab 漢字", 6);
    assert_eq!(text(&lines), ["ab", "漢字"]);
    assert_eq!(text(&render("漢字", 3)), ["漢", "字"]);
}

#[test]
fn graphemes_are_never_split() {
    let combining = "e\u{301}".repeat(5);
    let lines = render(&combining, 2);
    assert_eq!(
        text(&lines),
        ["e\u{301}e\u{301}", "e\u{301}e\u{301}", "e\u{301}"]
    );
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}".repeat(3);
    let lines = render(&family, 4);
    let joined: String = text(&lines).concat();
    assert_eq!(joined, family);
    for line in text(&lines) {
        assert!(
            !line.starts_with('\u{200D}'),
            "split a ZWJ sequence: {line:?}"
        );
    }
}

#[test]
fn char_wrap_breaks_at_exactly_the_width() {
    let style = Style::new();
    assert_eq!(text(&char_wrap("abcdefgh", 3, style)), ["abc", "def", "gh"]);
    assert_eq!(text(&char_wrap("abcdef", 3, style)), ["abc", "def"]);
}

#[test]
fn char_wrap_ignores_spaces_when_breaking() {
    assert_eq!(
        text(&char_wrap("ab cd ef gh", 4, Style::new())),
        ["ab c", "d ef", " gh"]
    );
    assert_eq!(
        text(&char_wrap("/usr/local/share/some/path", 8, Style::new())),
        ["/usr/loc", "al/share", "/some/pa", "th"]
    );
}

#[test]
fn char_wrap_keeps_indentation_and_blank_lines() {
    let lines = char_wrap(
        "  indented\n\n    deeper line here\ntail\n",
        8,
        Style::new(),
    );
    assert_eq!(
        text(&lines),
        ["  indent", "ed", "", "    deep", "er line ", "here", "tail"]
    );
    assert!(char_wrap("", 8, Style::new()).is_empty());
}

#[test]
fn char_wrap_measures_wide_characters_as_two_cells() {
    let style = Style::new();
    assert_eq!(text(&char_wrap("a漢b", 2, style)), ["a", "漢", "b"]);
    assert_eq!(text(&char_wrap("a漢b", 3, style)), ["a漢", "b"]);
    assert_eq!(text(&char_wrap("漢字漢", 4, style)), ["漢字", "漢"]);
    assert_eq!(text(&char_wrap("漢字", 1, style)), ["漢", "字"]);
}

#[test]
fn char_wrap_applies_the_style_and_expands_tabs() {
    let style = Style::new().add_modifier(Modifier::BOLD);
    let lines = char_wrap("a\tb", 40, style);
    assert_eq!(runs(&lines[0]), [run("a    b", style)]);
}

#[test]
fn char_wrap_lines_never_exceed_the_width() {
    let src = "x 漢字 yz\n  some/long/path/to/a/file.txt  and more";
    for width in 2..=20u16 {
        for line in char_wrap(src, width, Style::new()) {
            assert!(width_of(&line) <= usize::from(width), "{width}: {line}");
        }
    }
}

#[test]
fn rust_tokens_use_the_syntax_tokens() {
    let theme = distinct_theme();
    let code = "fn main() {\n    let s = \"hi\"; // note\n    let n = 42;\n}";
    let lines = highlight_code(code, "rust", &theme);
    assert_eq!(text(&lines), code.lines().collect::<Vec<_>>());
    let style_of = |needle: &str| {
        lines
            .iter()
            .flat_map(|l| &l.spans)
            .find(|s| s.content.contains(needle))
            .unwrap_or_else(|| panic!("no span with {needle:?}"))
            .style
    };
    assert_eq!(style_of("fn"), theme.style(Token::SyntaxKeyword));
    assert_eq!(style_of("let"), theme.style(Token::SyntaxKeyword));
    assert_eq!(style_of("main"), theme.style(Token::SyntaxFunction));
    assert_eq!(style_of("\"hi\""), theme.style(Token::SyntaxString));
    assert_eq!(style_of("// note"), theme.style(Token::SyntaxComment));
    assert_eq!(style_of("42"), theme.style(Token::SyntaxNumber));
    assert_eq!(style_of("()"), theme.style(Token::SyntaxPlain));
}

#[test]
fn the_seven_syntax_styles_of_the_test_theme_differ() {
    let theme = distinct_theme();
    let mut styles: Vec<_> = [
        Token::SyntaxKeyword,
        Token::SyntaxType,
        Token::SyntaxFunction,
        Token::SyntaxString,
        Token::SyntaxNumber,
        Token::SyntaxComment,
        Token::SyntaxPlain,
    ]
    .iter()
    .map(|t| format!("{:?}", theme.style(*t).fg))
    .collect();
    styles.sort();
    styles.dedup();
    assert_eq!(styles.len(), 7);
}

#[test]
fn a_comment_keeps_its_punctuation_and_a_string_its_quotes() {
    let theme = distinct_theme();
    let lines = highlight_code("// a comment\nlet s = \"x\";", "rs", &theme);
    assert_eq!(
        runs(&lines[0]),
        [run("// a comment", theme.style(Token::SyntaxComment))]
    );
    assert!(runs(&lines[1]).contains(&run("\"x\"", theme.style(Token::SyntaxString))));
}

#[test]
fn language_names_and_extensions_both_select_a_syntax() {
    let theme = distinct_theme();
    let code = "fn f() -> u8 { 7 }";
    let by_name = highlight_code(code, "rust", &theme);
    assert_eq!(by_name, highlight_code(code, "rs", &theme));
    assert_eq!(by_name, highlight_code(code, "Rust", &theme));
    assert_ne!(by_name, highlight_code(code, "", &theme));
}

#[test]
fn an_unknown_language_is_plain() {
    let theme = distinct_theme();
    for lang in ["", "nosuchlang", "  "] {
        let lines = highlight_code("fn x() {\n\n  1 }\n", lang, &theme);
        assert_eq!(text(&lines), ["fn x() {", "", "  1 }"]);
        for line in &lines {
            assert!(
                line.spans
                    .iter()
                    .all(|s| s.style == theme.style(Token::SyntaxPlain))
            );
        }
    }
}

#[test]
fn highlighting_does_not_wrap_and_has_no_trailing_line() {
    let theme = Theme::default();
    let long = "x".repeat(300);
    assert_eq!(highlight_code(&long, "rust", &theme).len(), 1);
    assert_eq!(highlight_code("a\n", "rust", &theme).len(), 1);
    assert_eq!(highlight_code("a\n\n", "rust", &theme).len(), 2);
    assert!(highlight_code("", "rust", &theme).is_empty());
}

#[test]
fn multi_line_constructs_keep_their_state_across_lines() {
    let theme = distinct_theme();
    let lines = highlight_code("/* one\n two */ fn", "rust", &theme);
    let comment = theme.style(Token::SyntaxComment);
    assert_eq!(runs(&lines[0]), [run("/* one", comment)]);
    assert_eq!(runs(&lines[1])[0], run(" two */", comment));
}

#[test]
fn scopes_map_to_tokens_by_the_innermost_match() {
    use std::str::FromStr;
    use syntect::parsing::ScopeStack;
    let token = |scopes: &str| highlight::token_for(&ScopeStack::from_str(scopes).unwrap());
    assert_eq!(token("source.x keyword.control.x"), Token::SyntaxKeyword);
    assert_eq!(token("source.x storage.type.x"), Token::SyntaxKeyword);
    assert_eq!(token("source.x entity.name.type.x"), Token::SyntaxType);
    assert_eq!(token("source.x support.type.x"), Token::SyntaxType);
    assert_eq!(
        token("source.x entity.name.function.x"),
        Token::SyntaxFunction
    );
    assert_eq!(token("source.x support.function.x"), Token::SyntaxFunction);
    assert_eq!(token("source.x string.quoted.x"), Token::SyntaxString);
    assert_eq!(token("source.x constant.numeric.x"), Token::SyntaxNumber);
    assert_eq!(token("source.x constant.language.x"), Token::SyntaxNumber);
    assert_eq!(token("source.x comment.line.x"), Token::SyntaxComment);
    assert_eq!(token("source.x variable.other.x"), Token::SyntaxPlain);
    assert_eq!(token("source.x"), Token::SyntaxPlain);
    assert_eq!(token(""), Token::SyntaxPlain);
    // The innermost scope with a rule wins, and scopes without one fall
    // through to the enclosing scope.
    assert_eq!(
        token("source.x string.quoted.x constant.character.escape.x"),
        Token::SyntaxString
    );
    assert_eq!(
        token("source.x comment.block.x keyword.todo.x"),
        Token::SyntaxKeyword
    );
    assert_eq!(
        token("source.x string.quoted.x punctuation.definition.string.begin.x"),
        Token::SyntaxString
    );
}

#[test]
fn changing_the_theme_changes_the_colours() {
    let default = Theme::default();
    let nord = builtin::get("nord").expect("nord is built in");
    let a = render_markdown(SAMPLE, 80, base_of(&default), &default);
    let b = render_markdown(SAMPLE, 80, base_of(&nord), &nord);
    assert_eq!(text(&a), text(&b));
    assert_ne!(a, b);
    let colours = |lines: &[Line]| -> std::collections::BTreeSet<String> {
        lines
            .iter()
            .flat_map(|l| &l.spans)
            .map(|s| format!("{:?}", s.style.fg))
            .collect()
    };
    assert_ne!(colours(&a), colours(&b));
}

#[test]
fn every_colour_comes_from_the_theme() {
    for id in ["default", "nord", "gruvbox", "catppuccin"] {
        let theme = builtin::get(id).expect("built-in theme");
        let allowed: Vec<_> = Role::ALL
            .iter()
            .map(|r| theme.role(*r))
            .chain(Token::ALL.iter().filter_map(|t| theme.style(*t).fg))
            .collect();
        let lines = render_markdown(SAMPLE, 60, base_of(&theme), &theme);
        for span in lines.iter().flat_map(|l| &l.spans) {
            if let Some(fg) = span.style.fg {
                assert!(
                    allowed.contains(&fg),
                    "{id}: {span:?} is not a theme colour"
                );
            }
            assert_eq!(span.style.bg, None, "{id}: {span:?}");
        }
    }
}

#[test]
fn the_base_style_is_the_starting_point_of_every_element() {
    let theme = Theme::default();
    let base = theme.style(Token::Thinking).add_modifier(Modifier::DIM);
    let lines = render_markdown("plain *em* [l](u)\n\n> quote", 40, base, &theme);
    assert_eq!(
        runs(&lines[0]),
        [
            run("plain ", base),
            run("em", base.add_modifier(Modifier::ITALIC)),
            run(" ", base),
            run("l", base.add_modifier(Modifier::UNDERLINED)),
        ]
    );
    assert_eq!(runs(&lines[2])[1], run("quote", base));
}

#[test]
fn rendering_is_deterministic() {
    let theme = Theme::default();
    let first = render_markdown(SAMPLE, 50, base_of(&theme), &theme);
    for _ in 0..3 {
        assert_eq!(render_markdown(SAMPLE, 50, base_of(&theme), &theme), first);
    }
}

#[test]
fn the_syntax_set_is_loaded_once() {
    let first: *const _ = highlight::syntax_set();
    let theme = Theme::default();
    for _ in 0..5 {
        render_markdown(SAMPLE, 40, base_of(&theme), &theme);
        highlight_code("fn main() {}", "rust", &theme);
    }
    assert!(std::ptr::eq(first, highlight::syntax_set()));
    assert_eq!(highlight::syntax_set_loads(), 1);
}

#[test]
fn no_line_is_wider_than_the_width() {
    let theme = Theme::default();
    for width in 9..=100u16 {
        for line in render_markdown(SAMPLE, width, base_of(&theme), &theme) {
            assert!(
                width_of(&line) <= usize::from(width),
                "width {width}: {line}"
            );
        }
    }
}

#[test]
fn text_and_code_fit_down_to_the_narrowest_width() {
    let src =
        "# Head 漢字\n\nsome prose with 漢字 and /a/long/path\n\n```rust\nfn main() {}\n```\n\n---";
    for width in 3..=40u16 {
        for line in render(src, width) {
            assert!(
                width_of(&line) <= usize::from(width),
                "width {width}: {line}"
            );
        }
    }
}

#[test]
fn the_sample_document_contains_every_element() {
    let all = text(&render(SAMPLE, 80)).join("\n");
    assert!(!all.contains('#'));
    assert!(!all.contains('`'));
    for needle in [
        "• First bullet",
        "  • Nested bullet one",
        "1. Ordered one",
        "│ A quoted",
        "─┼─",
        "  fn main() {",
    ] {
        assert!(all.contains(needle), "missing {needle:?}");
    }
}

// The widget.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

use crate::test_support::{buffer_to_styled, buffer_to_text, render as render_frame};

const CODE_ONLY: &str = "\
```rust
// Compute the answer.
fn answer(input: &str) -> u32 {
    let label = \"the answer\";
    if input.is_empty() { return 42; }
    label.len() as u32
}
```
";

/// Draws `src` in a `width` x `height` buffer with the default theme.
fn draw(src: &str, width: u16, height: u16) -> Buffer {
    let theme = Theme::default();
    render_frame(width, height, |frame| {
        frame.render_widget(Markdown::new(src, &theme), frame.area());
    })
}

/// The number of lines `src` takes at `width`, so a snapshot shows all of it.
fn height_at(src: &str, width: u16) -> u16 {
    u16::try_from(render(src, width).len()).unwrap()
}

#[test]
fn sample_document_at_80_columns_text() {
    let buffer = draw(SAMPLE, 80, height_at(SAMPLE, 80));
    insta::assert_snapshot!(buffer_to_text(&buffer));
}

#[test]
fn sample_document_at_80_columns_styled() {
    let buffer = draw(SAMPLE, 80, height_at(SAMPLE, 80));
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

#[test]
fn sample_document_at_40_columns_wraps_and_truncates() {
    let buffer = draw(SAMPLE, 40, height_at(SAMPLE, 40));
    insta::assert_snapshot!(buffer_to_text(&buffer));
}

#[test]
fn code_block_only_at_60_columns_styled() {
    let buffer = draw(CODE_ONLY, 60, height_at(CODE_ONLY, 60));
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

#[test]
fn the_widget_draws_what_render_markdown_returns() {
    let buffer = draw("hello *world*", 20, 2);
    assert_eq!(
        buffer_to_text(&buffer),
        "hello world         \n                    \n"
    );
}

#[test]
fn the_widget_cuts_off_lines_beyond_its_height() {
    let buffer = draw("one\n\ntwo\n\nthree", 10, 3);
    assert_eq!(
        buffer_to_text(&buffer),
        "one       \n          \ntwo       \n"
    );
}

#[test]
fn the_widget_draws_inside_its_area_and_wraps_to_its_width() {
    let theme = Theme::default();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 14, 4));
    Markdown::new("alpha beta gamma delta", &theme).render(Rect::new(2, 1, 10, 3), &mut buffer);
    assert_eq!(
        buffer_to_text(&buffer),
        "              \n  alpha beta  \n  gamma       \n  delta       \n"
    );
}

#[test]
fn the_widget_rewraps_when_the_area_is_resized() {
    let src = "one two three four five six";
    let wide = text(&render(src, 30));
    let narrow = text(&render(src, 10));
    assert_eq!(wide.len(), 1);
    assert_eq!(narrow, ["one two", "three four", "five six"]);
    let buffer = draw(src, 10, 3);
    assert_eq!(
        buffer_to_text(&buffer),
        "one two   \nthree four\nfive six  \n"
    );
}

#[test]
fn the_widget_base_style_sets_the_text_style() {
    let theme = Theme::default();
    let style = theme.style(Token::Thinking);
    let mut buffer = Buffer::empty(Rect::new(0, 0, 6, 1));
    Markdown::new("think", &theme)
        .base(style)
        .render(Rect::new(0, 0, 6, 1), &mut buffer);
    assert_eq!(buffer[(0, 0)].fg, style.fg.unwrap());
}
