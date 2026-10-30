//! The matcher and the search over turns.

use std::sync::Arc;

use serde_json::json;

use super::*;
use crate::components::turn_block::SectionKey;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{
    ByteSource, Item, ItemId, LineFormat, OutputSource, ToolOutput, ToolResult, ToolStatus, TurnId,
};

fn query(text: &str) -> SearchQuery {
    SearchQuery::new(text).unwrap()
}

fn found(q: &str, haystack: &str) -> Vec<String> {
    query(q)
        .find_all(haystack)
        .into_iter()
        .map(|r| haystack[r].to_owned())
        .collect()
}

// -- the matcher

#[test]
fn a_lower_case_query_matches_any_case() {
    assert_eq!(
        found("foo", "foo Foo FOO fOo"),
        ["foo", "Foo", "FOO", "fOo"]
    );
    assert!(!query("foo").is_case_sensitive());
}

#[test]
fn a_query_with_an_upper_case_letter_matches_that_case_only() {
    assert_eq!(found("Foo", "foo Foo FOO fOo"), ["Foo"]);
    assert!(query("Foo").is_case_sensitive());
    assert_eq!(found("fOO", "foo Foo FOO fOo"), Vec::<String>::new());
}

#[test]
fn an_empty_query_is_no_query() {
    assert!(SearchQuery::new("").is_none());
}

#[test]
fn matching_is_a_plain_substring_not_a_pattern() {
    assert_eq!(found("a.c", "abc a.c"), ["a.c"]);
    assert_eq!(found("a b", "a  b a b"), ["a b"]);
    assert_eq!(found("[x]", "x [x]"), ["[x]"]);
}

#[test]
fn occurrences_do_not_overlap_and_come_in_order() {
    let ranges = query("aa").find_all("aaaaa");
    assert_eq!(ranges, [0..2, 2..4]);
    assert_eq!(query("ab").find_all("ab ab"), [0..2, 3..5]);
}

#[test]
fn ranges_are_boundaries_of_the_original_text_whatever_the_folding_does() {
    // `İ` is 2 bytes and lower-cases to `i` + a combining dot; the Kelvin
    // sign is 3 bytes and lower-cases to `k`.
    let text = "İstanbul \u{212A}elvin café";
    for q in ["i", "k", "elvin", "café", "stan"] {
        for range in query(q).find_all(text) {
            assert!(text.is_char_boundary(range.start), "{q}: {range:?}");
            assert!(text.is_char_boundary(range.end), "{q}: {range:?}");
        }
    }
    assert_eq!(found("k", text), ["\u{212A}"]);
    assert_eq!(found("stan", text), ["stan"]);
    assert_eq!(found("CAFÉ", text), Vec::<String>::new());
    assert_eq!(found("café", "CAFÉ"), ["CAFÉ"]);
}

#[test]
fn a_query_longer_than_the_text_matches_nothing() {
    assert!(query("abcdef").find_all("abc").is_empty());
    assert!(!query("abcdef").is_match("abc"));
    assert!(query("b").is_match("abc"));
}

// -- what is searched

const TURN: &str = "t1";

fn corpus(turns: Vec<Turn>, show_system: bool) -> SearchCorpus {
    SearchCorpus::new(turns.into_iter().map(Arc::new).collect(), show_system)
}

fn with_id(turn: Turn, id: &str) -> Turn {
    crate::components::transcript::fixtures::with_id(turn, id)
}

fn run(q: &str, corpus: &SearchCorpus) -> Vec<Match> {
    search(&query(q), corpus)
}

fn keys(matches: &[Match]) -> Vec<SectionKey> {
    matches.iter().map(|m| m.section.key.clone()).collect()
}

fn shell_call(id: &str, input: serde_json::Value, output: ToolOutput) -> Item {
    Item::ToolCall(crate::model::ToolCall {
        id: ItemId::from(id),
        tool: crate::model::ToolKind::Shell,
        description: "Look around".to_owned(),
        input,
        result: Some(ToolResult {
            output,
            is_error: false,
        }),
        status: ToolStatus::Ok,
        duration: None,
    })
}

/// A lazily stored output of more than 4 KiB with `needle` on its last line,
/// as a Claude transcript line holds it.
fn lazy_output(call_id: &str, needle: &str) -> (ToolOutput, usize) {
    let text = format!("{}\n{needle}", "padding line\n".repeat(400));
    let line = json!({"message": {"content": [
        {"type": "tool_result", "tool_use_id": call_id, "content": text}
    ]}})
    .to_string();
    let len = line.len() as u64;
    let source = OutputSource {
        bytes: ByteSource::Memory(Arc::from(line.into_bytes())),
        range: 0..len,
        format: LineFormat::Claude,
        call_id: call_id.to_owned(),
    };
    let lines = text.lines().count();
    let output = ToolOutput::from_text(text, || source);
    assert!(output.is_lazy());
    (output, lines)
}

#[test]
fn folded_sections_are_searched_with_their_full_text() {
    // A User prompt is folded by default and shows two lines; the match is
    // on the tenth.
    let prompt = format!("{}the needle is here", "a line\n".repeat(9));
    let corpus = corpus(vec![with_id(blocks::user(&prompt, vec![]), TURN)], true);
    let matches = run("needle", &corpus);
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].section.key, SectionKey::Prompt);
    assert_eq!(matches[0].line, 9);
    assert_eq!(matches[0].range, 4..10);
}

#[test]
fn tool_input_description_and_output_are_searched() {
    let call = shell_call(
        "c1",
        json!({"command": "grep -r in_the_input .", "timeout": 120000}),
        ToolOutput::inline("one\nin_the_output\nthree".to_owned()),
    );
    let corpus = corpus(
        vec![with_id(blocks::assistant(None, vec![call], None), "a1")],
        true,
    );
    let in_input = run("in_the_input", &corpus);
    let in_output = run("in_the_output", &corpus);
    let in_description = run("look around", &corpus);
    let in_a_number = run("120000", &corpus);
    for matches in [&in_input, &in_output, &in_description, &in_a_number] {
        assert_eq!(matches.len(), 1, "{matches:?}");
        assert_eq!(matches[0].section.key, SectionKey::Item(ItemId::from("c1")));
    }
    // The description is the first line, the command the second, ...
    assert_eq!(in_description[0].line, 0);
    assert!(in_input[0].line > in_description[0].line);
    assert!(in_output[0].line > in_input[0].line);
    // Keys of the input are not text of the transcript.
    assert!(run("timeout", &corpus).is_empty());
}

#[test]
fn strings_of_the_input_are_searched_as_they_are() {
    let call = shell_call(
        "c1",
        json!({"edits": [{"old": "say \"hi\"\nnext", "new": "x"}]}),
        ToolOutput::inline(String::new()),
    );
    let corpus = corpus(
        vec![with_id(blocks::assistant(None, vec![call], None), "a1")],
        true,
    );
    let matches = run("say \"hi\"", &corpus);
    assert_eq!(matches.len(), 1);
    let next = run("next", &corpus);
    assert_eq!(next[0].line, matches[0].line + 1);
}

#[test]
fn a_lazy_output_is_read_back_and_searched_to_its_end() {
    let (output, lines) = lazy_output("c1", "the lazy needle");
    let call = shell_call("c1", json!({"command": "cat big.log"}), output);
    let corpus = corpus(
        vec![with_id(blocks::assistant(None, vec![call], None), "a1")],
        true,
    );
    let matches = run("lazy needle", &corpus);
    assert_eq!(matches.len(), 1, "{matches:?}");
    // The description and the command come first, then the output.
    assert_eq!(matches[0].line, 2 + lines - 1);
    assert_eq!(matches[0].range, 4..15);
}

#[test]
fn an_output_that_cannot_be_read_is_skipped_without_failing_the_search() {
    let broken = ToolOutput::lazy(
        10,
        50_000,
        OutputSource {
            bytes: ByteSource::Memory(Arc::from(&b"{}"[..])),
            range: 0..99,
            format: LineFormat::Claude,
            call_id: "c1".to_owned(),
        },
    );
    let call = shell_call("c1", json!({"command": "needle"}), broken);
    let corpus = corpus(
        vec![with_id(blocks::assistant(None, vec![call], None), "a1")],
        true,
    );
    assert_eq!(run("needle", &corpus).len(), 1);
}

#[test]
fn system_content_is_searched_while_it_is_shown_and_not_when_hidden() {
    let injected = blocks::injected("i1", &[("file", "notes with a needle")]);
    let turns = vec![with_id(blocks::user("a prompt", vec![injected]), TURN)];
    let shown = run("needle", &corpus(turns.clone(), true));
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].section.key, SectionKey::Item(ItemId::from("i1")));
    let hidden = run("needle", &corpus(turns, false));
    assert!(hidden.is_empty(), "{hidden:?}");
}

#[test]
fn compaction_summaries_stats_and_model_changes_are_searched() {
    let summary = with_id(blocks::compaction("a summary about needles"), "k1");
    let stats = with_id(
        blocks::assistant(
            None,
            vec![Item::ModelChange {
                id: ItemId::from("m1"),
                model: "claude-sonnet-5.5".to_owned(),
            }],
            Some(blocks::stats()),
        ),
        "a1",
    );
    let corpus = corpus(vec![summary, stats], true);
    let found = run("needles", &corpus);
    assert_eq!(keys(&found), [SectionKey::Summary]);
    assert_eq!(
        keys(&run("sonnet", &corpus)),
        [SectionKey::Item(ItemId::from("m1"))]
    );
    assert_eq!(keys(&run("112 messages", &corpus)), [SectionKey::Footer]);
}

#[test]
fn several_matches_in_a_section_are_separate_matches_in_display_order() {
    let corpus = corpus(
        vec![
            with_id(blocks::user("usb usb\nand USB", vec![]), "t1"),
            with_id(
                blocks::assistant(None, vec![blocks::text("x1", "a usb here")], None),
                "t2",
            ),
        ],
        true,
    );
    let matches = run("usb", &corpus);
    let at: Vec<(String, usize, std::ops::Range<usize>)> = matches
        .iter()
        .map(|m| (m.section.turn.as_str().to_owned(), m.line, m.range.clone()))
        .collect();
    assert_eq!(
        at,
        [
            ("t1".to_owned(), 0, 0..3),
            ("t1".to_owned(), 0, 4..7),
            ("t1".to_owned(), 1, 4..7),
            ("t2".to_owned(), 0, 2..5),
        ]
    );
    // Smart case on the same text.
    assert_eq!(run("USB", &corpus).len(), 1);
}

#[test]
fn a_request_runs_the_search_and_carries_its_generation_back() {
    let corpus = corpus(vec![with_id(blocks::user("a needle", vec![]), TURN)], true);
    let request = SearchRequest {
        generation: 7,
        query: "needle".to_owned(),
        corpus,
    };
    let answer = request.run();
    assert_eq!(answer.generation(), 7);
    assert_eq!(answer.matches().len(), 1);
    // Answers are equal when they answer the same request.
    assert_eq!(answer, SearchMatches::new(7, Vec::new()));
    assert_ne!(answer, SearchMatches::new(8, Vec::new()));
    // The turn ids are those of the corpus.
    assert_eq!(answer.matches()[0].section.turn, TurnId::from(TURN));
}

#[test]
fn the_position_reads_as_current_over_total() {
    assert_eq!(
        SearchPosition {
            current: 3,
            total: 17
        }
        .to_string(),
        "3/17"
    );
    assert_eq!(
        SearchPosition {
            current: 0,
            total: 0
        }
        .to_string(),
        "0/0"
    );
}
