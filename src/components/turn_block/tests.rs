//! Behaviour tests of sections, renderers and the block geometry.

use std::time::Duration;

use ratatui::style::{Color, Modifier};
use ratatui::widgets::Widget;
use serde_json::json;
use unicode_width::UnicodeWidthStr;

use super::fixtures::*;
use super::renderers::{
    CompactionSection, SectionCtx, SectionRenderer, StatsSection, SystemSection, ThinkingSection,
    ToolCallSection, UserPromptSection, format_stats,
};
use super::*;
use crate::model::{Item, ItemId, Thinking, TokenUsage, ToolStatus, Turn, TurnStats};
use crate::test_support::buffer_to_text;
use crate::theme::{Role, Token};

fn kinds(turn: &Turn, show_system: bool) -> Vec<SectionKind> {
    derive_sections(turn, show_system)
        .iter()
        .map(Section::kind)
        .collect()
}

// ---- Section derivation ----

#[test]
fn user_turn_lists_injected_items_before_the_prompt() {
    let turn = user("Hi", vec![injected("i1", &[("file", "a")])]);
    assert_eq!(
        kinds(&turn, true),
        [SectionKind::System, SectionKind::UserPrompt]
    );
    let ids: Vec<_> = derive_sections(&turn, true)
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids[0], item_id(&turn, "i1"));
    assert_eq!(ids[1], key_id(&turn, SectionKey::Prompt));
}

#[test]
fn injected_items_yield_no_section_when_show_system_is_false() {
    let turn = user("Hi", vec![injected("i1", &[("file", "a")])]);
    assert_eq!(kinds(&turn, false), [SectionKind::UserPrompt]);
    let turn = assistant(
        None,
        vec![injected("i1", &[("file", "a")]), text("t1", "x")],
        None,
    );
    assert_eq!(kinds(&turn, false), [SectionKind::AssistantText]);
    assert_eq!(
        kinds(&turn, true),
        [SectionKind::System, SectionKind::AssistantText]
    );
}

#[test]
fn every_item_becomes_its_section_kind_and_stats_come_last() {
    let turn = full_assistant();
    assert_eq!(
        kinds(&turn, true),
        [
            SectionKind::AssistantText,
            SectionKind::Thinking,
            SectionKind::ToolCall,
            SectionKind::ToolCall,
            SectionKind::ModelChange,
            SectionKind::Stats,
        ]
    );
    let last = derive_sections(&turn, true).pop().unwrap();
    assert_eq!(last.id, key_id(&turn, SectionKey::Footer));
}

#[test]
fn redacted_thinking_yields_no_section() {
    let turn = assistant(
        None,
        vec![
            thinking("th1", Thinking::Redacted),
            thinking("th2", Thinking::Text("  \n".into())),
            text("t1", "x"),
        ],
        None,
    );
    assert_eq!(kinds(&turn, true), [SectionKind::AssistantText]);
    let only = assistant(None, vec![thinking("th1", Thinking::Redacted)], None);
    assert!(kinds(&only, true).is_empty());
}

#[test]
fn compaction_turn_has_one_summary_section() {
    let turn = compaction("summary");
    let sections = derive_sections(&turn, true);
    assert_eq!(sections.len(), 1);
    assert_eq!(sections[0].kind(), SectionKind::Compaction);
    assert_eq!(sections[0].id, key_id(&turn, SectionKey::Summary));
}

#[test]
fn default_folds_follow_the_section_table() {
    let user_turn = user("p", vec![injected("i1", &[("file", "a")])]);
    let folds = FoldState::new();
    for section in derive_sections(&user_turn, true) {
        assert!(folds.is_folded(&section), "{:?}", section.kind());
    }
    let turn = full_assistant();
    let folded: Vec<(SectionKind, bool)> = derive_sections(&turn, true)
        .iter()
        .map(|s| (s.kind(), folds.is_folded(s)))
        .collect();
    assert_eq!(
        folded,
        [
            (SectionKind::AssistantText, false),
            (SectionKind::Thinking, true),
            (SectionKind::ToolCall, true),
            (SectionKind::ToolCall, true),
            (SectionKind::ModelChange, false),
            (SectionKind::Stats, false),
        ]
    );
    let compaction_turn = compaction("s");
    assert!(folds.is_folded(&derive_sections(&compaction_turn, true)[0]));
}

#[test]
fn fold_state_overrides_foldable_sections_only() {
    let turn = full_assistant();
    let mut folds = FoldState::new();
    folds.set_folded(item_id(&turn, "c1"), false);
    folds.set_folded(item_id(&turn, "t1"), true);
    let sections = derive_sections(&turn, true);
    assert!(!folds.is_folded(&sections[2]));
    assert!(folds.is_folded(&sections[3]));
    assert!(!folds.is_folded(&sections[0]), "assistant text never folds");
}

// ---- Renderers ----

fn ctx(theme: &crate::theme::Theme, width: u16, folded: bool) -> SectionCtx<'_> {
    SectionCtx::new(theme, width, folded)
}

fn lines_text(lines: &[ratatui::text::Line<'_>]) -> Vec<String> {
    lines.iter().map(ToString::to_string).collect()
}

#[test]
fn folded_prompt_shows_two_lines_and_an_ellipsis_when_longer() {
    let theme = theme();
    let lines = UserPromptSection(LONG_PROMPT).lines(&ctx(&theme, 72, true));
    let text = lines_text(&lines);
    assert_eq!(text.len(), 2);
    assert!(text[0].starts_with("▶ I have a Logitech"));
    assert!(text[1].starts_with("  works fine"));
    assert!(text[1].ends_with('…'));
    assert!(!text[0].ends_with('…'));
    assert!(lines.iter().all(|l| l.width() <= 74));
}

#[test]
fn folded_prompt_without_more_lines_has_no_ellipsis() {
    let theme = theme();
    for prompt in ["short", "one  \ntwo"] {
        let lines = UserPromptSection(prompt).lines(&ctx(&theme, 72, true));
        assert!(
            lines.iter().all(|l| !l.to_string().contains('…')),
            "{prompt:?}"
        );
    }
    let two = UserPromptSection("one  \ntwo").lines(&ctx(&theme, 72, true));
    assert_eq!(lines_text(&two), ["▶ one", "  two"]);
}

#[test]
fn folded_prompt_of_three_lines_ends_the_second_with_an_ellipsis() {
    let theme = theme();
    let lines = UserPromptSection("one  \ntwo  \nthree").lines(&ctx(&theme, 72, true));
    assert_eq!(lines_text(&lines), ["▶ one", "  two…"]);
}

#[test]
fn folded_prompt_ellipsis_is_truncated_to_fit_a_full_line() {
    let theme = theme();
    let prompt = format!("{}  \n{}  \nthird", "a".repeat(10), "b".repeat(20));
    let lines = UserPromptSection(&prompt).lines(&ctx(&theme, 10, true));
    let second = lines[1].to_string();
    assert_eq!(second, "  bbbbbbbbb…");
    assert_eq!(second.width(), 12);
}

#[test]
fn unfolded_prompt_shows_every_line() {
    let theme = theme();
    let folded = UserPromptSection(LONG_PROMPT).lines(&ctx(&theme, 72, true));
    let unfolded = UserPromptSection(LONG_PROMPT).lines(&ctx(&theme, 72, false));
    assert_eq!(unfolded.len(), 8);
    assert!(unfolded[0].to_string().starts_with("▼ "));
    assert!(unfolded.iter().all(|l| !l.to_string().contains('…')));
    assert!(unfolded.len() > folded.len());
}

#[test]
fn thinking_folded_and_unfolded_use_the_thinking_token() {
    let theme = distinct_theme();
    let style = theme.style(Token::Thinking);
    let folded = ThinkingSection("\nFirst line  \nSecond").lines(&ctx(&theme, 60, true));
    assert_eq!(lines_text(&folded), ["▶ Thinking: First line"]);
    assert!(folded[0].spans.iter().all(|s| s.style == style));
    let unfolded = ThinkingSection("First line  \nSecond").lines(&ctx(&theme, 60, false));
    assert_eq!(
        lines_text(&unfolded),
        ["▼ Thinking", "  First line", "  Second"]
    );
    assert!(
        unfolded
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| s.style == style)
    );
}

#[test]
fn folded_thinking_is_truncated_to_one_row() {
    let theme = theme();
    let lines = ThinkingSection(&"word ".repeat(40)).lines(&ctx(&theme, 30, true));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].width(), 32);
    assert!(lines[0].to_string().ends_with('…'));
}

#[test]
fn tool_call_folded_shows_the_description_in_its_token() {
    let theme = distinct_theme();
    let turn = full_assistant();
    let Turn::Assistant(a) = &turn else { panic!() };
    let crate::model::Item::ToolCall(call) = &a.items[2] else {
        panic!()
    };
    let lines = ToolCallSection(call).lines(&ctx(&theme, 100, true));
    assert_eq!(
        lines_text(&lines),
        ["▶ Find the BRIO on USB and check available tools"]
    );
    let style = theme.style(Token::ToolDescription);
    assert!(lines[0].spans.iter().all(|s| s.style == style));
}

#[test]
fn tool_call_unfolded_has_its_body_in_its_tokens() {
    let theme = distinct_theme();
    let turn = assistant(
        None,
        vec![tool_of(
            crate::model::ToolKind::Other("mcp__x__y".into()),
            "c1",
            "Run it",
            json!({"command": "ls"}),
            Some(("out", false)),
            ToolStatus::Ok,
        )],
        None,
    );
    let Turn::Assistant(a) = &turn else { panic!() };
    let crate::model::Item::ToolCall(call) = &a.items[0] else {
        panic!()
    };
    let lines = ToolCallSection(call).lines(&ctx(&theme, 60, false));
    assert_eq!(
        lines_text(&lines),
        [
            "▼ Run it",
            "",
            "  Input:",
            "",
            "     {",
            "       \"command\": \"ls\"",
            "     }",
            "",
            "  Result:",
            "",
            "     out",
        ]
    );
    let color = |row: usize, idx: usize| lines[row].spans[idx].style.fg.unwrap();
    let last = |row: usize| lines[row].spans.last().unwrap().style.fg.unwrap();
    assert_eq!(color(0, 1), theme.role(Role::Primary));
    assert_eq!(color(2, 1), theme.role(Role::Warning));
    assert_eq!(last(4), theme.role(Role::Accent));
    assert_eq!(color(8, 1), theme.role(Role::Warning));
    assert_eq!(last(10), theme.role(Role::Secondary));
}

#[test]
fn tool_result_is_error_output_when_the_status_is_error() {
    let theme = distinct_theme();
    let turn = assistant(
        None,
        vec![tool(
            "c1",
            "d",
            json!({}),
            Some(("boom", false)),
            ToolStatus::Error,
        )],
        None,
    );
    let Turn::Assistant(a) = &turn else { panic!() };
    let crate::model::Item::ToolCall(call) = &a.items[0] else {
        panic!()
    };
    let lines = ToolCallSection(call).lines(&ctx(&theme, 60, false));
    let last = lines.last().unwrap();
    assert_eq!(last.to_string(), "     boom");
    let text = last.spans.last().unwrap();
    assert_eq!(text.style.fg, Some(theme.role(Role::Destructive)));
}

#[test]
fn pending_tool_shows_muted_pending_at_column_7() {
    let theme = distinct_theme();
    let turn = assistant(
        None,
        vec![tool("c1", "d", json!({}), None, ToolStatus::Pending)],
        None,
    );
    let Turn::Assistant(a) = &turn else { panic!() };
    let crate::model::Item::ToolCall(call) = &a.items[0] else {
        panic!()
    };
    let lines = ToolCallSection(call).lines(&ctx(&theme, 60, false));
    let last = lines.last().unwrap();
    assert_eq!(last.to_string(), "     (pending)");
    assert_eq!(
        last.spans.last().unwrap().style.fg,
        Some(theme.role(Role::MutedForeground))
    );
}

#[test]
fn compaction_folded_and_unfolded() {
    let theme = distinct_theme();
    let muted = Some(theme.role(Role::MutedForeground));
    let folded = CompactionSection("The *summary*.").lines(&ctx(&theme, 60, true));
    assert_eq!(lines_text(&folded), ["▶ ── context compacted ──"]);
    let unfolded = CompactionSection("The *summary*.").lines(&ctx(&theme, 60, false));
    assert_eq!(
        lines_text(&unfolded),
        ["▼ ── context compacted ──", "  The summary."]
    );
    assert!(
        folded
            .iter()
            .chain(&unfolded)
            .flat_map(|l| &l.spans)
            .all(|s| s.style.fg == muted)
    );
}

#[test]
fn system_section_headers_and_unfolded_entries() {
    let theme = distinct_theme();
    let one = [entry("file", "\n  /a/b.txt  \nmore")];
    let folded = SystemSection(&one).lines(&ctx(&theme, 60, true));
    assert_eq!(lines_text(&folded), ["▶ file: /a/b.txt"]);
    let unfolded = SystemSection(&one).lines(&ctx(&theme, 60, false));
    assert_eq!(
        lines_text(&unfolded),
        [
            "▼ file: /a/b.txt",
            "  file",
            "",
            "       /a/b.txt  ",
            "     more"
        ]
    );
    let many = [entry("file", "a"), entry("hook", "b"), entry("meta", "c")];
    let folded = SystemSection(&many).lines(&ctx(&theme, 60, true));
    assert_eq!(lines_text(&folded), ["▶ 3 injected items"]);
    let unfolded = SystemSection(&many).lines(&ctx(&theme, 60, false));
    assert_eq!(unfolded[0].to_string(), "▼ 3 injected items");
    assert!(
        unfolded
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| s.style.fg == Some(theme.role(Role::MutedForeground)))
    );
}

fn entry(kind: &str, text: &str) -> crate::model::InjectedEntry {
    crate::model::InjectedEntry {
        kind: kind.to_owned(),
        timestamp: ts("2026-10-21T08:35:10Z"),
        text: text.to_owned(),
    }
}

// ---- Stats footer ----

#[test]
fn stats_footer_formats_the_spec_example() {
    let stats = TurnStats {
        duration: Duration::from_millis(302_646),
        message_count: 112,
        tokens: Some(TokenUsage {
            input: 5_213,
            output: 40_000,
            ..TokenUsage::default()
        }),
        cost: None,
    };
    assert_eq!(format_stats(&stats), "5m 2s · 112 messages · 45.2k tokens");
}

#[test]
fn the_footer_total_is_input_plus_output_plus_cache_writes() {
    let usage = TokenUsage {
        input: 1_000,
        output: 2_000,
        cache_read: 400_000,
        cache_write: 30_000,
        reasoning: 500,
    };
    // Cache reads are shown apart and reasoning is already in `output`.
    assert_eq!(usage.total(), 33_000);
}

#[test]
fn the_spec_example_figures_give_1_5m_tokens_and_359_0m_cached() {
    let mut stats = stats();
    stats.tokens = Some(TokenUsage {
        input: 2_800,
        output: 336_000,
        cache_write: 1_200_000,
        cache_read: 359_000_000,
        reasoning: 90_000,
    });
    assert_eq!(
        format_stats(&stats),
        "5m 2s · 112 messages · 1.5M tokens · 359.0M cached"
    );
}

#[test]
fn stats_footer_shows_cached_after_tokens_and_hides_it_when_zero() {
    let stats = stats_with_cache(359_400_000, None);
    assert_eq!(
        format_stats(&stats),
        "5m 2s · 112 messages · 45.2k tokens · 359.4M cached"
    );
    assert_eq!(
        format_stats(&stats_with_cache(0, None)),
        "5m 2s · 112 messages · 45.2k tokens"
    );
    assert_eq!(
        format_stats(&stats_with_cache(1, None)),
        "5m 2s · 112 messages · 45.2k tokens · 1 cached"
    );
}

#[test]
fn stats_footer_keeps_the_cost_last_after_cached() {
    assert_eq!(
        format_stats(&stats_with_cache(359_400_000, Some(0.42))),
        "5m 2s · 112 messages · 45.2k tokens · 359.4M cached · $0.42"
    );
    assert_eq!(
        format_stats(&stats_with_cache(0, Some(0.42))),
        "5m 2s · 112 messages · 45.2k tokens · $0.42"
    );
}

#[test]
fn a_turn_without_usage_shows_neither_tokens_nor_cached() {
    let mut stats = stats_with_cache(359_400_000, None);
    stats.tokens = None;
    assert_eq!(format_stats(&stats), "5m 2s · 112 messages");
}

#[test]
fn a_single_total_token_is_singular() {
    let mut stats = stats();
    stats.tokens = Some(TokenUsage {
        output: 1,
        ..TokenUsage::default()
    });
    assert_eq!(format_stats(&stats), "5m 2s · 112 messages · 1 token");
}

/// The text of the cells drawn with the `search_match` background, row by
/// row.
fn highlighted_rows(buf: &ratatui::buffer::Buffer, bg: Color) -> Vec<(u16, String)> {
    let mut found = Vec::new();
    for y in 0..buf.area.height {
        let run: String = (0..buf.area.width)
            .map(|x| &buf[(x, y)])
            .filter(|cell| cell.bg == bg)
            .map(|cell| cell.symbol())
            .collect();
        if !run.is_empty() {
            found.push((y, run));
        }
    }
    found
}

#[test]
fn searching_the_cached_part_highlights_it_on_the_footer_row() {
    let turn = assistant(
        None,
        vec![text("t1", "Done.")],
        Some(stats_with_cache(359_400_000, None)),
    );
    let opts = Opts::default();
    let query = crate::transcript_search::SearchQuery::new("359.4M cached").unwrap();

    // The search finds the footer section...
    let corpus =
        crate::transcript_search::SearchCorpus::new(vec![std::sync::Arc::new(turn.clone())], true);
    let found = crate::transcript_search::search(&query, &corpus);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].section.key, SectionKey::Footer);

    // ... and the footer row shows exactly the matched text highlighted.
    let bg = opts.theme.role(Role::Primary);
    let buf = with_block(&turn, &opts, |block| {
        crate::test_support::render(80, height(&turn, &opts, 80) as u16, |frame| {
            block
                .search(Some(&query))
                .render(frame.area(), frame.buffer_mut());
        })
    });
    let text = buffer_to_text(&buf);
    let rows: Vec<&str> = text.lines().collect();
    let highlighted = highlighted_rows(&buf, bg);
    assert_eq!(highlighted.len(), 1, "{highlighted:?}");
    let (y, run) = &highlighted[0];
    assert_eq!(run, "359.4M cached");
    assert!(
        rows[usize::from(*y)].contains("45.2k tokens · 359.4M cached"),
        "{}",
        rows[usize::from(*y)]
    );
}

#[test]
fn a_cached_part_that_is_hidden_cannot_be_found() {
    let turn = assistant(
        None,
        vec![text("t1", "Done.")],
        Some(stats_with_cache(0, None)),
    );
    let corpus = crate::transcript_search::SearchCorpus::new(vec![std::sync::Arc::new(turn)], true);
    let query = crate::transcript_search::SearchQuery::new("cached").unwrap();
    assert!(crate::transcript_search::search(&query, &corpus).is_empty());
}

#[test]
fn stats_footer_adds_cost_and_omits_unknown_parts() {
    let mut stats = stats();
    stats.cost = Some(0.4);
    assert_eq!(
        format_stats(&stats),
        "5m 2s · 112 messages · 45.2k tokens · $0.40"
    );
    stats.tokens = None;
    stats.cost = None;
    assert_eq!(format_stats(&stats), "5m 2s · 112 messages");
    stats.message_count = 0;
    assert_eq!(format_stats(&stats), "5m 2s");
}

#[test]
fn footer_is_muted_and_indented_by_two_spaces() {
    let theme = distinct_theme();
    let stats = stats();
    let lines = StatsSection(&stats).lines(&ctx(&theme, 60, false));
    assert_eq!(
        lines_text(&lines),
        ["  5m 2s · 112 messages · 45.2k tokens"]
    );
    assert!(
        lines[0]
            .spans
            .iter()
            .all(|s| s.style.fg == Some(theme.role(Role::MutedForeground)))
    );
}

// ---- Block geometry ----

#[test]
fn model_change_is_at_column_4() {
    let turn = assistant(
        None,
        vec![Item::ModelChange {
            id: ItemId::from("m1"),
            model: "claude-sonnet-5.5".into(),
        }],
        None,
    );
    for width in [80, 120] {
        let rows = rows(&draw(&turn, &Opts::default(), width));
        assert_eq!(cols(&rows[2], 0, 4), "│   ");
        assert_eq!(cols(&rows[2], 4, 29), "model → claude-sonnet-5.5");
        assert_eq!(
            draw(&turn, &Opts::default(), width)[(4, 2)].fg,
            theme().role(Role::MutedForeground)
        );
    }
}

#[test]
fn titles_have_the_planner_notes_format() {
    for width in [80usize, 120] {
        let w = u16::try_from(width).unwrap();
        let user_rows = rows(&draw(&user("x", vec![]), &Opts::default(), w));
        let right = " 2026-10-21 08:35:15Z ─╮";
        let left = "╭─ User ";
        assert_eq!(
            user_rows[0],
            format!(
                "{left}{}{right}",
                "─".repeat(width - left.chars().count() - right.chars().count())
            )
        );
        let a = assistant(Some("high"), vec![], None);
        let rows_a = rows(&draw(&a, &Opts::default(), w));
        let left = "╭─ Assistant ── claude-opus-5.5 @ high ";
        let right = " 2026-10-21 08:35:18Z ─╮";
        assert_eq!(
            rows_a[0],
            format!(
                "{left}{}{right}",
                "─".repeat(width - left.chars().count() - right.chars().count())
            )
        );
        let rows_c = rows(&draw(&compaction("s"), &Opts::default(), w));
        assert!(rows_c[0].starts_with("╭─ Compaction ─"));
        assert!(rows_c[0].ends_with(" 2026-10-21 09:00:00Z ─╮"));
        assert_eq!(rows_c[0].chars().count(), width);
    }
}

#[test]
fn title_colours_come_from_their_tokens() {
    let opts = Opts::default().themed(distinct_theme());
    let theme = distinct_theme();
    let buf = draw(&assistant(Some("high"), vec![], None), &opts, 80);
    assert_eq!(buf[(0, 0)].fg, theme.role(Role::Border));
    assert_eq!(buf[(3, 0)].fg, theme.role(Role::Accent), "Assistant");
    assert_eq!(buf[(13, 0)].fg, theme.role(Role::Border), "the joiner");
    assert_eq!(buf[(14, 0)].fg, theme.role(Role::Border));
    assert_eq!(buf[(16, 0)].fg, theme.role(Role::Accent), "model");
    assert_eq!(buf[(58, 0)].fg, theme.role(Role::Destructive), "timestamp");
    assert_eq!(buf[(79, 0)].fg, theme.role(Role::Border));
    let user_buf = draw(&user("x", vec![]), &opts, 80);
    assert_eq!(user_buf[(3, 0)].fg, theme.role(Role::Primary));
    let comp = draw(&compaction("x"), &opts, 80);
    assert_eq!(comp[(3, 0)].fg, theme.role(Role::Warning));
}

#[test]
fn rows_are_blank_sections_separated_by_blanks_and_a_blank_before_the_bottom() {
    let turn = full_assistant();
    for width in [80u16, 120] {
        let rows = rows(&draw(&turn, &Opts::default(), width));
        let blank = format!("│{}│", " ".repeat(usize::from(width) - 2));
        assert_eq!(rows[1], blank);
        assert_eq!(rows[rows.len() - 2], blank);
        let layout = with_block(&turn, &Opts::default(), |b| b.layout(width));
        let spans = layout.section_rows();
        assert_eq!(spans[0].1.start, 2);
        for pair in spans.windows(2) {
            assert_eq!(pair[0].1.end + 1, pair[1].1.start);
            assert_eq!(rows[pair[0].1.end], blank);
        }
        assert_eq!(spans.last().unwrap().1.end, rows.len() - 2);
    }
}

#[test]
fn text_starts_at_column_4_and_wraps_at_w_minus_5() {
    let turn = user(&"x".repeat(400), vec![]);
    let opts = Opts::default().unfold(key_id(&turn, SectionKey::Prompt));
    for width in [80usize, 120] {
        let rows = rows(&draw(&turn, &opts, u16::try_from(width).unwrap()));
        assert_eq!(cols(&rows[2], 0, 4), "│ ▼ ");
        assert_eq!(cols(&rows[3], 0, 4), "│   ");
        for row in &rows[2..rows.len() - 3] {
            let text = cols(row, 4, width - 4);
            assert_eq!(text.chars().count(), width - 8);
            assert!(text.chars().all(|c| c == 'x'), "full width rows: {row}");
            assert_eq!(cols(row, width - 4, width), "   │");
        }
        let last = &rows[rows.len() - 3];
        assert!(last.chars().nth(4).unwrap() == 'x');
    }
}

#[test]
fn prose_wraps_so_that_no_text_passes_column_w_minus_5() {
    let turn = assistant(None, vec![text("t1", LONG_PROMPT)], None);
    for width in [80usize, 120] {
        let rows = rows(&draw(
            &turn,
            &Opts::default(),
            u16::try_from(width).unwrap(),
        ));
        for row in &rows[2..rows.len() - 3] {
            assert_eq!(cols(row, 0, 4), "│   ");
            assert_eq!(cols(row, width - 4, width), "   │");
            assert!(!cols(row, 4, 5).trim().is_empty());
        }
    }
}

#[test]
fn tool_body_labels_are_at_column_4_and_content_at_column_7() {
    let turn = full_assistant();
    let opts = Opts::default().unfold(item_id(&turn, "c2"));
    for width in [80usize, 120] {
        let rows = rows(&draw(&turn, &opts, u16::try_from(width).unwrap()));
        let at = |needle: &str| rows.iter().position(|r| r.contains(needle)).unwrap();
        let input = at("Command:");
        let result = at("Result:");
        assert_eq!(cols(&rows[input], 0, 13), "│   Command: ");
        assert_eq!(cols(&rows[input - 1], 1, 4), "   ");
        assert_eq!(rows[input + 1].trim_matches(['│', ' ']), "");
        assert_eq!(cols(&rows[input + 2], 0, 8), "│      s");
        assert_eq!(cols(&rows[input + 2], 7, 11), "sudo");
        assert_eq!(cols(&rows[result], 4, 11), "Result:");
        assert_eq!(rows[result + 1].trim_matches(['│', ' ']), "");
        assert_eq!(cols(&rows[result + 2], 7, 11), "sudo");
        // Content is char-wrapped at W - 11 cells.
        for row in &rows[input + 2..result - 1] {
            assert_eq!(cols(row, width - 4, width), "   │");
        }
        let long = rows
            .iter()
            .filter(|r| r.chars().nth(7).is_some_and(|c| c != ' ') && r.contains("claude-1000"))
            .count();
        assert!(long > 0);
    }
}

#[test]
fn tool_body_wraps_json_to_the_body_width() {
    let turn = assistant(
        None,
        vec![tool_of(
            crate::model::ToolKind::Other("mcp__x__y".into()),
            "c1",
            "d",
            json!({"k": "y".repeat(300)}),
            Some(("z".repeat(200).as_str(), false)),
            ToolStatus::Ok,
        )],
        None,
    );
    let opts = Opts::default().unfold(item_id(&turn, "c1"));
    for width in [80usize, 120] {
        let rows = rows(&draw(&turn, &opts, u16::try_from(width).unwrap()));
        let full: Vec<&String> = rows.iter().filter(|r| r.contains("yyyy")).collect();
        assert!(full.len() >= 3);
        // First JSON line `  "k": "yyy…`, so only the continuation rows are full.
        assert_eq!(cols(full[1], 7, width - 4).chars().count(), width - 11);
        assert!(cols(full[1], 7, width - 4).chars().all(|c| c == 'y'));
        let zs: Vec<&String> = rows.iter().filter(|r| r.contains("zzzz")).collect();
        assert_eq!(cols(zs[0], 7, width - 4), "z".repeat(width - 11));
    }
}

#[test]
fn an_empty_block_is_three_rows() {
    let turn = assistant(None, vec![thinking("t", Thinking::Redacted)], None);
    let buf = draw(&turn, &Opts::default(), 80);
    let rows = rows(&buf);
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1], format!("│{}│", " ".repeat(78)));
    assert_eq!(height(&turn, &Opts::default(), 80), 3);
}

#[test]
fn show_system_false_removes_the_injected_rows_from_the_height() {
    let turn = user("Hi", vec![injected("i1", &[("file", "a")])]);
    let shown = height(&turn, &Opts::default(), 80);
    let hidden = height(
        &turn,
        &Opts {
            show_system: false,
            ..Opts::default()
        },
        80,
    );
    assert_eq!(shown, hidden + 2);
}

#[test]
fn height_equals_the_rows_drawn_for_many_widths_and_fold_states() {
    let turns = [
        user(
            LONG_PROMPT,
            vec![injected("i1", &[("a", "x"), ("b", "y\nz")])],
        ),
        full_assistant(),
        compaction("one\n\ntwo `code`"),
        assistant(Some("low"), vec![thinking("t", Thinking::Redacted)], None),
    ];
    for turn in &turns {
        let ids: Vec<SectionId> = derive_sections(turn, true)
            .into_iter()
            .map(|s| s.id)
            .collect();
        for unfolded in [false, true] {
            let mut opts = Opts::default();
            for id in &ids {
                opts.folds.set_folded(id.clone(), !unfolded);
            }
            for width in 8..=130 {
                // `draw` checks the bottom border sits at row `height - 1`.
                draw(turn, &opts, width);
            }
        }
    }
}

#[test]
fn tiny_widths_do_not_panic() {
    let turn = full_assistant();
    for width in 0..9 {
        let _ = height(&turn, &Opts::default(), width);
        let _ = render_into(&turn, &Opts::default(), width, 12, 0);
    }
}

// ---- Clipping ----

#[test]
fn clipped_rendering_draws_the_matching_rows_of_the_full_rendering() {
    let turn = full_assistant();
    let opts = Opts::default()
        .unfold(item_id(&turn, "c2"))
        .select(item_id(&turn, "c2"));
    let width = 100;
    let h = height(&turn, &opts, width);
    let full = draw(&turn, &opts, width);
    for (skip, limit) in [
        (0, 5),
        (1, 3),
        (3, 9),
        (14, 10),
        (h - 4, 4),
        (h - 1, 1),
        (7, h),
    ] {
        let clipped = render_into(&turn, &opts, width, u16::try_from(limit).unwrap(), skip);
        for dy in 0..limit {
            for x in 0..width {
                let expected = if skip + dy < h {
                    full[(x, u16::try_from(skip + dy).unwrap())].clone()
                } else {
                    ratatui::buffer::Cell::default()
                };
                assert_eq!(
                    clipped[(x, u16::try_from(dy).unwrap())],
                    expected,
                    "skip {skip} limit {limit} cell ({x}, {dy})"
                );
            }
        }
    }
}

#[test]
fn skipping_past_the_end_draws_nothing() {
    let turn = user("x", vec![]);
    let h = height(&turn, &Opts::default(), 80);
    let buf = render_into(&turn, &Opts::default(), 80, 4, h);
    assert!(buffer_to_text(&buf).lines().all(|l| l.trim().is_empty()));
}

// ---- Selection and dimming ----

fn selected_rows(buf: &ratatui::buffer::Buffer) -> Vec<u16> {
    (0..buf.area.height)
        .filter(|&y| buf[(0, y)].symbol() == "▒")
        .collect()
}

#[test]
fn the_marker_covers_exactly_the_rows_of_the_selected_section() {
    let turn = full_assistant();
    let width = 120;
    for id in ["t1", "th1", "c1", "c2", "m1"] {
        let opts = Opts::default()
            .unfold(item_id(&turn, id))
            .select(item_id(&turn, id));
        let buf = draw(&turn, &opts, width);
        let layout = with_block(&turn, &opts, |b| b.layout(width));
        let (_, range) = layout
            .section_rows()
            .iter()
            .find(|(sid, _)| *sid == item_id(&turn, id))
            .unwrap();
        let expected: Vec<u16> = range.clone().map(|r| u16::try_from(r).unwrap()).collect();
        assert_eq!(selected_rows(&buf), expected, "section {id}");
        for &y in &expected {
            assert_eq!(buf[(0, y)].fg, theme().role(Role::Primary));
        }
    }
}

#[test]
fn an_unfolded_tool_selection_covers_the_description_through_the_last_body_row() {
    let turn = full_assistant();
    let opts = Opts::default()
        .unfold(item_id(&turn, "c2"))
        .select(item_id(&turn, "c2"));
    let rows = rows(&draw(&turn, &opts, 120));
    let marked: Vec<usize> = (0..rows.len())
        .filter(|&i| rows[i].starts_with('▒'))
        .collect();
    assert!(rows[marked[0]].contains("▼ Dump full USB descriptors"));
    assert!(rows[*marked.last().unwrap()].contains("lsusb.txt"));
    assert_eq!(marked.len(), marked.last().unwrap() - marked[0] + 1);
    assert_eq!(rows[marked[0] - 1].chars().next(), Some('│'));
    assert_eq!(rows[marked.last().unwrap() + 1].chars().next(), Some('│'));
}

#[test]
fn no_selection_draws_no_marker() {
    let buf = draw(&full_assistant(), &Opts::default(), 120);
    assert!(selected_rows(&buf).is_empty());
}

#[test]
fn selected_text_is_bold_with_its_token_colours_and_others_are_not() {
    let turn = full_assistant();
    let theme = distinct_theme();
    let opts = Opts::default()
        .themed(theme.clone())
        .dim(false)
        .unfold(item_id(&turn, "c2"))
        .select(item_id(&turn, "c2"));
    let width = 120;
    let buf = draw(&turn, &opts, width);
    let layout = with_block(&turn, &opts, |b| b.layout(width));
    let range = layout
        .section_rows()
        .iter()
        .find(|(sid, _)| *sid == item_id(&turn, "c2"))
        .unwrap()
        .1
        .clone();
    for y in 0..buf.area.height {
        let in_section = range.contains(&usize::from(y));
        for x in 2..width - 4 {
            let cell = &buf[(x, y)];
            if cell.symbol().trim().is_empty() {
                continue;
            }
            assert_eq!(
                cell.modifier.contains(Modifier::BOLD),
                in_section,
                "({x}, {y}) {}",
                cell.symbol()
            );
        }
    }
    let first = usize::from(u16::try_from(range.start).unwrap());
    assert_eq!(
        buf[(4, u16::try_from(first).unwrap())].fg,
        theme.role(Role::Primary)
    );
    // `Command:` label keeps its colour; the command text its token.
    assert_eq!(
        buf[(4, u16::try_from(first + 2).unwrap())].fg,
        theme.role(Role::Warning)
    );
    assert_eq!(
        buf[(7, u16::try_from(first + 4).unwrap())].fg,
        theme.role(Role::Accent)
    );
}

fn text_colours(buf: &ratatui::buffer::Buffer, width: u16) -> Vec<Color> {
    let mut out = Vec::new();
    for y in 1..buf.area.height - 1 {
        for x in 2..width - 4 {
            let cell = &buf[(x, y)];
            if !cell.symbol().trim().is_empty() {
                out.push(cell.fg);
            }
        }
    }
    out
}

#[test]
fn dim_unselected_mutes_every_other_section_but_not_titles_or_borders() {
    let turn = full_assistant();
    let theme = distinct_theme();
    let muted = theme.role(Role::MutedForeground);
    let opts = Opts::default()
        .themed(theme.clone())
        .unfold(item_id(&turn, "c2"))
        .select(item_id(&turn, "c2"));
    let width = 120;
    let buf = draw(&turn, &opts, width);
    let layout = with_block(&turn, &opts, |b| b.layout(width));
    for (sid, range) in layout.section_rows() {
        for y in range.clone() {
            for x in 2..width - 4 {
                let cell = &buf[(x, u16::try_from(y).unwrap())];
                if cell.symbol().trim().is_empty() {
                    continue;
                }
                let dimmed = cell.fg == muted;
                assert_eq!(dimmed, *sid != item_id(&turn, "c2"), "({x}, {y})");
            }
        }
    }
    // Titles and borders keep their colours.
    assert_eq!(buf[(3, 0)].fg, theme.role(Role::Accent));
    assert_eq!(buf[(98, 0)].fg, theme.role(Role::Destructive));
    assert_eq!(buf[(0, 0)].fg, theme.role(Role::Border));
    assert_eq!(buf[(119, 3)].fg, theme.role(Role::Border));
    assert_eq!(buf[(0, 3)].fg, theme.role(Role::Border));
}

#[test]
fn dimming_keeps_modifiers() {
    let turn = assistant(None, vec![text("t1", "plain **strong** plain")], None);
    let buf = draw(&turn, &Opts::default(), 40);
    let strong = &buf[(10, 2)];
    assert_eq!(strong.symbol(), "s");
    assert!(strong.modifier.contains(Modifier::BOLD));
    assert_eq!(strong.fg, theme().role(Role::MutedForeground));
}

#[test]
fn without_dimming_unselected_sections_keep_their_token_colours() {
    let turn = full_assistant();
    let theme = distinct_theme();
    let opts = Opts::default().themed(theme.clone()).dim(false);
    let width = 120;
    let buf = draw(&turn, &opts, width);
    let colours = text_colours(&buf, width);
    for role in [Role::Foreground, Role::PrimaryForeground, Role::Primary] {
        assert!(colours.contains(&theme.role(role)), "{role:?}");
    }
    assert!(
        colours.contains(&theme.role(Role::MutedForeground)),
        "footer"
    );
    assert!(selected_rows(&buf).is_empty());
    // Nothing is bold: only the selection makes text bold.
    let bold = (1..buf.area.height - 1)
        .flat_map(|y| (2..width - 4).map(move |x| (x, y)))
        .filter(|&(x, y)| buf[(x, y)].modifier.contains(Modifier::BOLD))
        .count();
    assert_eq!(bold, 0);
}

#[test]
fn thinking_gets_the_thinking_token_when_not_dimmed() {
    let turn = full_assistant();
    let theme = distinct_theme();
    let opts = Opts::default().themed(theme.clone()).dim(false);
    let buf = draw(&turn, &opts, 120);
    let layout = with_block(&turn, &opts, |b| b.layout(120));
    let row = layout.section_rows()[1].1.start;
    assert_eq!(
        buf[(4, u16::try_from(row).unwrap())].fg,
        theme.role(Role::PrimaryForeground)
    );
}

// ---- Round 2: markdown in folded thinking, prose wrapping, titles ----

fn tool_item(call: &Turn) -> &crate::model::ToolCall {
    let Turn::Assistant(a) = call else { panic!() };
    let Item::ToolCall(call) = &a.items[0] else {
        panic!()
    };
    call
}

#[test]
fn folded_thinking_shows_the_rendered_first_line_not_markdown_source() {
    let theme = theme();
    let show = |src: &str| ThinkingSection(src).lines(&ctx(&theme, 70, true))[0].to_string();
    assert_eq!(
        show("**Investigating USB descriptors**\n\nBody"),
        "▶ Thinking: Investigating USB descriptors"
    );
    assert_eq!(
        show("- first bullet\n- second"),
        "▶ Thinking: • first bullet"
    );
    assert_eq!(show("## Plan\n\ntext"), "▶ Thinking: Plan");
    assert_eq!(show("Use `lsusb` *now*"), "▶ Thinking: Use lsusb now");
    let long =
        ThinkingSection(&format!("**{}**", "word ".repeat(40))).lines(&ctx(&theme, 30, true));
    assert_eq!(long.len(), 1);
    assert_eq!(long[0].width(), 32);
    assert!(long[0].to_string().ends_with('…'));
}

#[test]
fn folded_thinking_stays_in_the_thinking_token() {
    let theme = distinct_theme();
    let lines = ThinkingSection("**Bold** start").lines(&ctx(&theme, 60, true));
    let style = theme.style(Token::Thinking);
    assert!(lines[0].spans.iter().all(|s| s.style == style));
}

#[test]
fn unfolded_tool_description_is_word_wrapped_at_the_text_width() {
    let theme = theme();
    let description = "Dump the full USB descriptors for the Logitech BRIO and list every extension unit with its controls";
    let turn = assistant(
        None,
        vec![tool(
            "c1",
            description,
            json!({}),
            None,
            ToolStatus::Pending,
        )],
        None,
    );
    let lines = ToolCallSection(tool_item(&turn)).lines(&ctx(&theme, 72, false));
    assert_eq!(
        lines_text(&lines[..2]),
        [
            "▼ Dump the full USB descriptors for the Logitech BRIO and list every",
            "  extension unit with its controls",
        ]
    );
    let opts = Opts::default().unfold(item_id(&turn, "c1"));
    let rows = rows(&draw(&turn, &opts, 80));
    assert_eq!(cols(&rows[3], 0, 6), "│   ex");
    let narrow = ToolCallSection(tool_item(&turn)).lines(&ctx(&theme, 32, false));
    for line in &narrow[..4] {
        assert!(line.width() <= 34);
        assert!(!line.to_string().starts_with("   "), "{line}");
    }
    // A single token longer than the width is still broken.
    let long = assistant(
        None,
        vec![tool(
            "c1",
            &"x".repeat(100),
            json!({}),
            None,
            ToolStatus::Ok,
        )],
        None,
    );
    let lines = ToolCallSection(tool_item(&long)).lines(&ctx(&theme, 40, false));
    assert_eq!(lines[0].width(), 42);
    assert!(lines[1].to_string().starts_with("  x"));
}

#[test]
fn blank_descriptions_fall_back_to_the_tool_name_and_tabs_agree() {
    let theme = theme();
    let blank = assistant(
        None,
        vec![tool("c1", " ", json!({}), None, ToolStatus::Ok)],
        None,
    );
    let folded = ToolCallSection(tool_item(&blank)).lines(&ctx(&theme, 40, true));
    assert_eq!(lines_text(&folded), ["▶ Shell"]);
    let tabbed = assistant(
        None,
        vec![tool("c1", "Run\tthings", json!({}), None, ToolStatus::Ok)],
        None,
    );
    let folded = ToolCallSection(tool_item(&tabbed)).lines(&ctx(&theme, 40, true));
    let unfolded = ToolCallSection(tool_item(&tabbed)).lines(&ctx(&theme, 40, false));
    assert_eq!(folded[0].to_string(), "▶ Run things");
    assert_eq!(unfolded[0].to_string(), "▼ Run things");
}

#[test]
fn folded_prompt_skips_blank_separator_rows() {
    let theme = distinct_theme();
    let fg = theme.role(Role::Foreground);
    let two = UserPromptSection("Para one.\n\nPara two.").lines(&ctx(&theme, 60, true));
    assert_eq!(lines_text(&two), ["▶ Para one.", "  Para two."]);
    let three =
        UserPromptSection("Para one.\n\nPara two.\n\nPara three.").lines(&ctx(&theme, 60, true));
    assert_eq!(lines_text(&three), ["▶ Para one.", "  Para two.…"]);
    assert!(three[1].spans.iter().all(|s| s.style.fg == Some(fg)));
    let heading =
        UserPromptSection("# Bug report\n\nIt breaks.\n\nMore.").lines(&ctx(&theme, 60, true));
    assert_eq!(lines_text(&heading), ["▶ Bug report", "  It breaks.…"]);
    // Unfolded keeps the blank rows.
    let unfolded = UserPromptSection("Para one.\n\nPara two.").lines(&ctx(&theme, 60, false));
    assert_eq!(lines_text(&unfolded), ["▼ Para one.", "", "  Para two."]);
}

#[test]
fn long_model_is_cut_before_the_timestamp_is_dropped() {
    let mut turn = assistant(Some("xhigh"), vec![], None);
    if let Turn::Assistant(a) = &mut turn {
        a.model = "claude-opus-5.5-20261021-extended-context-preview-1m".into();
    }
    for width in [80usize, 60, 50] {
        let top = rows(&draw(
            &turn,
            &Opts::default(),
            u16::try_from(width).unwrap(),
        ))[0]
            .clone();
        assert_eq!(top.chars().count(), width);
        assert!(top.starts_with("╭─ Assistant ── claude-"), "{top}");
        assert!(top.ends_with(" 08:35:18Z ─╮"), "{top}");
    }
    let top = rows(&draw(&turn, &Opts::default(), 60))[0].clone();
    assert!(top.contains("… ─"), "{top}");
    let wide = rows(&draw(&turn, &Opts::default(), 120))[0].clone();
    assert!(wide.contains("-1m @ xhigh "), "no cut when it fits: {wide}");
    // Without room for the model the timestamp goes, then the left title is cut.
    let top = rows(&draw(&turn, &Opts::default(), 30))[0].clone();
    assert_eq!(top.chars().count(), 30);
    assert!(top.ends_with('╮'));
    assert!(!top.contains("08:35"));
}

#[test]
fn the_top_border_is_always_the_width_and_ends_in_a_corner() {
    let turns = [user("x", vec![]), full_assistant(), compaction("s")];
    for turn in &turns {
        let formatter = formatter();
        let theme = theme();
        let bar = TitleBar::new(turn, &theme, &formatter);
        for width in 2..=140u16 {
            let line = bar.line(width).to_string();
            assert_eq!(line.width(), usize::from(width), "{line}");
            assert!(line.starts_with('╭') && line.ends_with('╮'), "{line}");
        }
    }
}

#[test]
fn stats_footer_and_model_note_wrap_at_word_boundaries() {
    let theme = theme();
    let stats = stats();
    let lines = StatsSection(&stats).lines(&ctx(&theme, 24, false));
    assert_eq!(
        lines_text(&lines),
        ["  5m 2s · 112 messages ·", "  45.2k tokens"]
    );
    let note = super::renderers::ModelChangeSection("claude-sonnet-5.5 preview mode");
    let lines = note.lines(&ctx(&theme, 30, false));
    assert_eq!(
        lines_text(&lines),
        ["  model → claude-sonnet-5.5", "  preview mode"]
    );
}

#[test]
fn a_wrapped_footer_row_never_starts_with_the_separator() {
    let theme = theme();
    let stats = stats_with_cache(359_400_000, Some(0.42));
    let lines = StatsSection(&stats).lines(&ctx(&theme, 58, false));
    assert_eq!(
        lines_text(&lines),
        [
            "  5m 2s · 112 messages · 45.2k tokens · 359.4M cached ·",
            "  $0.42"
        ]
    );
    let lines = StatsSection(&stats).lines(&ctx(&theme, 14, false));
    assert_eq!(
        lines_text(&lines),
        [
            "  5m 2s · 112",
            "  messages ·",
            "  45.2k tokens ·",
            "  359.4M",
            "  cached · $0.42"
        ]
    );
    // At any width that holds the longest part with its separator
    // (`messages ·`), no row starts with the separator, and the rows read as
    // the footer text once joined.
    for width in 10..=70 {
        let lines = StatsSection(&stats).lines(&ctx(&theme, width, false));
        let rows = lines_text(&lines);
        assert!(
            rows.iter().all(|r| !r.trim_start().starts_with('·')),
            "{rows:?}"
        );
        let joined = rows
            .iter()
            .map(|r| r.trim())
            .collect::<Vec<_>>()
            .join(" ")
            .replace("  ", " ");
        assert_eq!(joined, format_stats(&stats), "width {width}");
    }
}

#[test]
fn rendering_a_cached_layout_equals_rendering_the_block() {
    let turn = full_assistant();
    let opts = Opts::default()
        .unfold(item_id(&turn, "c2"))
        .select(item_id(&turn, "c2"));
    let width = 90;
    let direct = render_into(&turn, &opts, width, 40, 3);
    let cached = with_block(&turn, &opts, |block| {
        let block = block.skip_rows(3);
        let layout = block.layout(width);
        crate::test_support::render(width, 40, |frame| {
            block.render_layout(&layout, frame.area(), frame.buffer_mut());
        })
    });
    assert_eq!(direct, cached);
    let by_ref = with_block(&turn, &opts, |block| {
        let block = block.skip_rows(3);
        crate::test_support::render(width, 40, |frame| {
            (&block).render(frame.area(), frame.buffer_mut());
        })
    });
    assert_eq!(direct, by_ref);
}

#[test]
fn text_never_reaches_the_padding_at_tiny_widths() {
    let turn = assistant(None, vec![text("t1", "Hello there")], None);
    for width in 3..=8u16 {
        let rows = rows(&render_into(&turn, &Opts::default(), width, 6, 0));
        for row in &rows[1..rows.len() - 1] {
            let w = usize::from(width);
            assert!(
                row.chars()
                    .skip(w.saturating_sub(4))
                    .take(3)
                    .all(|c| c == ' ' || c == '│' || c == '╯'),
                "{row}"
            );
        }
    }
}

// ---- Task 019: truncation stops and the open_subagent hint ----

fn long_result_turn(lines: usize) -> Turn {
    let text: String = (1..=lines).map(|n| format!("row {n}\n")).collect();
    assistant(
        None,
        vec![
            tool(
                "c1",
                "Print the log",
                json!({"command": "cat log"}),
                Some((&text, false)),
                ToolStatus::Ok,
            ),
            tool(
                "c2",
                "Next",
                json!({"command": "true"}),
                Some(("ok", false)),
                ToolStatus::Ok,
            ),
        ],
        None,
    )
}

fn more_id(turn: &Turn, item: &str) -> SectionId {
    key_id(turn, SectionKey::More(crate::model::ItemId::from(item)))
}

#[test]
fn a_truncated_call_has_a_second_stop_directly_below_it() {
    let turn = long_result_turn(30);
    let opts = Opts::default().unfold(item_id(&turn, "c1")).truncate(10);
    let (stops, height) = with_block(&turn, &opts, |block| {
        let layout = block.layout(60);
        (layout.section_rows().to_vec(), layout.height())
    });
    let ids: Vec<SectionId> = stops.iter().map(|(id, _)| id.clone()).collect();
    assert_eq!(
        ids,
        [
            item_id(&turn, "c1"),
            more_id(&turn, "c1"),
            item_id(&turn, "c2")
        ]
    );
    assert_eq!(stops[0].1.end, stops[1].1.start, "no blank row between");
    assert_eq!(stops[1].1.len(), 1);
    assert_eq!(stops[1].1.end + 1, stops[2].1.start, "one blank row after");
    assert_eq!(height, draw(&turn, &opts, 60).area.height as usize);
}

#[test]
fn selecting_the_more_row_marks_that_row_only() {
    let turn = long_result_turn(30);
    let opts = Opts::default()
        .unfold(item_id(&turn, "c1"))
        .truncate(10)
        .select(more_id(&turn, "c1"));
    let rows = rows(&draw(&turn, &opts, 60));
    let marked: Vec<&String> = rows.iter().filter(|r| r.starts_with('▒')).collect();
    assert_eq!(marked.len(), 1);
    assert_eq!(marked[0].trim_matches(['▒', '│', ' ']), "… 20 more lines");
    // Selecting the tool section marks its rows but not the more row.
    let opts = Opts::default()
        .unfold(item_id(&turn, "c1"))
        .truncate(10)
        .select(item_id(&turn, "c1"));
    let rows = self::rows(&draw(&turn, &opts, 60));
    let more = rows.iter().find(|r| r.contains("more lines")).unwrap();
    assert!(more.starts_with('│'));
    assert!(rows.iter().filter(|r| r.starts_with('▒')).count() > 10);
}

#[test]
fn an_expanded_call_has_all_its_lines_and_no_more_row() {
    let turn = long_result_turn(30);
    let opts = Opts::default()
        .unfold(item_id(&turn, "c1"))
        .truncate(10)
        .expand(item_id(&turn, "c1"));
    let rows = rows(&draw(&turn, &opts, 60));
    assert!(rows.iter().any(|r| r.contains("row 30")));
    assert!(rows.iter().all(|r| !r.contains("more line")));
    let ids = with_block(&turn, &opts, |b| {
        b.layout(60)
            .section_rows()
            .iter()
            .map(|(id, _)| id.clone())
            .collect::<Vec<_>>()
    });
    assert_eq!(ids, [item_id(&turn, "c1"), item_id(&turn, "c2")]);
}

#[test]
fn a_folded_truncated_call_has_no_more_row() {
    let turn = long_result_turn(30);
    let opts = Opts::default().truncate(10);
    let ids = with_block(&turn, &opts, |b| b.layout(60).section_rows().len());
    assert_eq!(ids, 2);
}

#[test]
fn the_open_transcript_hint_follows_the_remapped_open_subagent_key() {
    use crate::config::Config;
    use crate::keymap::{Action, Context, Keymap};
    let subagent = |opts: &Opts| {
        let turn = assistant(
            None,
            vec![tool_of(
                crate::model::ToolKind::Subagent,
                "c1",
                "Explore",
                json!({"subagent_type": "Explore", "prompt": "Look"}),
                Some(("Done", false)),
                ToolStatus::Ok,
            )],
            None,
        );
        let opts = opts.clone().unfold(item_id(&turn, "c1"));
        rows(&draw(&turn, &opts, 80))
            .into_iter()
            .find(|r| r.contains("to open transcript"))
            .unwrap()
            .trim_matches(['│', ' '])
            .to_owned()
    };
    let label = |km: &Keymap| km.label(Context::Browser, Action::OpenSubagent).unwrap();
    let default = Keymap::default();
    assert_eq!(
        subagent(&Opts::default().open_key(&label(&default))),
        "a to open transcript"
    );
    let (config, warnings) = Config::from_toml_str(
        "[keys.browser]\nopen_subagent = [\"o\"]\n",
        std::path::Path::new("/home/tester"),
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let (remapped, _) = Keymap::from_config(&config.keys);
    assert_eq!(
        subagent(&Opts::default().open_key(&label(&remapped))),
        "o to open transcript"
    );
}

// ---- Notices ----

#[test]
fn a_notice_is_a_section_of_its_own_kind_in_the_turns_order() {
    use crate::model::NoticeLevel;
    let turn = assistant(
        None,
        vec![
            text("t1", "Hello."),
            notice("n1", NoticeLevel::Error, "boom"),
            text("t2", "After."),
        ],
        Some(stats()),
    );
    assert_eq!(
        kinds(&turn, true),
        [
            SectionKind::AssistantText,
            SectionKind::Notice,
            SectionKind::AssistantText,
            SectionKind::Stats
        ]
    );
}

#[test]
fn a_notice_cannot_fold_and_is_never_folded() {
    use crate::model::NoticeLevel;
    assert!(!SectionKind::Notice.foldable());
    assert!(!SectionKind::Notice.default_folded());
    let turn = assistant(None, vec![notice("n1", NoticeLevel::Error, "boom")], None);
    let section = derive_sections(&turn, true).remove(0);
    let mut folds = FoldState::new();
    assert!(!folds.is_folded(&section));
    folds.set_folded(section.id.clone(), true);
    assert!(!folds.is_folded(&section));
}

#[test]
fn a_notice_is_saved_as_error_or_notice() {
    use crate::model::NoticeLevel;
    let turn = assistant(
        None,
        vec![
            notice("n1", NoticeLevel::Error, "boom"),
            notice("n2", NoticeLevel::Warning, "aborted"),
        ],
        None,
    );
    let words: Vec<&str> = derive_sections(&turn, true)
        .iter()
        .map(Section::file_kind)
        .collect();
    assert_eq!(words, ["error", "notice"]);
    assert_eq!(
        super::plain::file_kind(&turn, &item_id(&turn, "n2").clone()),
        Some("notice")
    );
}

#[test]
fn a_notice_without_text_is_not_a_section() {
    use crate::model::NoticeLevel;
    let turn = assistant(None, vec![notice("n1", NoticeLevel::Error, "  \n")], None);
    assert!(derive_sections(&turn, true).is_empty());
}

#[test]
fn a_notice_uses_the_level_style_two_spaces_in_and_wraps_to_the_text_width() {
    use crate::model::NoticeLevel;
    use crate::theme::Token;
    let theme = theme();
    let ctx = SectionCtx::new(&theme, 12, false);
    let lines =
        super::renderers::NoticeSection(NoticeLevel::Error, "alpha beta gamma delta").lines(&ctx);
    let rows: Vec<String> = lines.iter().map(ToString::to_string).collect();
    assert_eq!(rows, ["  alpha beta", "  gamma delta"]);
    let error = theme.style(Token::ErrorOutput).fg;
    assert!(
        lines
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| s.style.fg == error)
    );
    let muted = SectionCtx::new(&theme, 12, false).muted().fg;
    let warning = super::renderers::NoticeSection(NoticeLevel::Warning, "aborted").lines(&ctx);
    assert!(
        warning
            .iter()
            .flat_map(|l| &l.spans)
            .all(|s| s.style.fg == muted)
    );
    assert_ne!(error, muted);
}

#[test]
fn an_assembled_claude_api_error_is_drawn_as_one_notice_in_error_output() {
    use crate::harness::claude::Assembler;
    use std::sync::Arc;

    let transcript = concat!(
        r#"{"type":"user","uuid":"u1","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"go"}}"#,
        "\n",
        r#"{"type":"assistant","uuid":"a1","isApiErrorMessage":true,"timestamp":"2026-01-01T00:00:01Z","message":{"id":"msg_e","model":"<synthetic>","stop_reason":null,"content":[{"type":"text","text":"API Error: 529 overloaded"}]}}"#,
        "\n",
    );
    let turns = Assembler::from_memory(Arc::from(transcript.as_bytes())).into_turns();
    let turn = (*turns[1]).clone();

    assert_eq!(kinds(&turn, true), [SectionKind::Notice]);
    let theme = distinct_theme();
    let buffer = draw(&turn, &Opts::default().dim(false).themed(theme.clone()), 60);
    let rendered = buffer_to_text(&buffer);
    assert_eq!(
        rendered.matches("API Error: 529 overloaded").count(),
        1,
        "{rendered}"
    );
    let error = theme.style(Token::ErrorOutput).fg;
    // The cells of the notice's own row: each letter is in `error_output`.
    let width = usize::from(buffer.area.width);
    let row = buffer
        .content()
        .chunks(width)
        .find(|row| {
            row.iter()
                .map(|c| c.symbol())
                .collect::<String>()
                .contains("API Error")
        })
        .expect("a row with the notice");
    assert!(
        row.iter()
            .filter(|cell| !cell.symbol().trim().is_empty() && cell.symbol() != "│")
            .all(|cell| Some(cell.fg) == error),
        "the notice text is drawn in error_output"
    );
}
