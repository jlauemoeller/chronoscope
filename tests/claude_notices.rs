//! Notices for Claude API errors and unusual stop reasons (cycle 2 SPEC §4.2),
//! on constructed lines: `max_tokens` and `refusal` occur in no local
//! transcript.

use std::sync::Arc;

use chronoscope::harness::claude::Assembler;
use chronoscope::model::{AssistantTurn, ByteSource, Item, NoticeLevel, ToolStatus, Turn};

const PROMPT: &str = r#"{"type":"user","uuid":"u1","origin":{"kind":"human"},"timestamp":"2026-01-01T00:00:00Z","message":{"role":"user","content":"go"}}"#;

/// One `assistant` line of message `msg`. `stop` is the raw JSON of
/// `stop_reason`, `blocks` the raw JSON of the content blocks and `extra`
/// further top-level keys (`"isApiErrorMessage":true,`, say).
fn line_with(n: u32, msg: &str, model: &str, stop: &str, blocks: &str, extra: &str) -> String {
    format!(
        r#"{{"type":"assistant","uuid":"a{n}",{extra}"timestamp":"2026-01-01T00:00:{n:02}Z","message":{{"id":"{msg}","model":"{model}","role":"assistant","stop_reason":{stop},"content":[{blocks}]}}}}"#
    )
}

fn text(t: &str) -> String {
    format!(r#"{{"type":"text","text":"{t}"}}"#)
}

fn tool_use(id: &str) -> String {
    format!(
        r#"{{"type":"tool_use","id":"{id}","name":"Bash","input":{{"command":"ls","description":"List"}}}}"#
    )
}

/// A line of message `msg` with the given stop reason (a JSON literal).
fn line(n: u32, msg: &str, stop: &str, blocks: &str) -> String {
    line_with(n, msg, "claude-sonnet-5-5", stop, blocks, "")
}

/// A line that carries an API error, flagged on the entry.
fn api_error_line(n: u32, msg: &str, stop: &str, body: &str) -> String {
    line_with(
        n,
        msg,
        "<synthetic>",
        stop,
        &text(body),
        r#""isApiErrorMessage":true,"#,
    )
}

fn result_line(n: u32, call: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"r{n}","timestamp":"2026-01-01T00:01:{n:02}Z","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{call}","content":"file.txt"}}]}}}}"#
    )
}

fn transcript(lines: &[String]) -> String {
    let mut out = String::from(PROMPT);
    for l in lines {
        out.push('\n');
        out.push_str(l);
    }
    out.push('\n');
    out
}

fn assemble(lines: &[String]) -> Assembler {
    Assembler::from_memory(Arc::from(transcript(lines).as_bytes()))
}

/// The items of the Assistant turn after the prompt.
fn items_of(asm: &Assembler) -> Vec<Item> {
    match &*asm.turns()[1] {
        Turn::Assistant(AssistantTurn { items, .. }) => items.clone(),
        other => panic!("expected an Assistant turn, got {other:?}"),
    }
}

fn notices(items: &[Item]) -> Vec<(String, NoticeLevel, String)> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Notice { id, level, text } => Some((id.to_string(), *level, text.clone())),
            _ => None,
        })
        .collect()
}

fn texts(items: &[Item]) -> Vec<String> {
    items
        .iter()
        .filter_map(|i| match i {
            Item::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The notices of a one-message transcript ending with `stop`.
fn notices_for_stop(stop: &str) -> Vec<(String, NoticeLevel, String)> {
    let asm = assemble(&[line(1, "msg_1", stop, &text("Partial."))]);
    notices(&items_of(&asm))
}

// ---------------------------------------------------------- API errors

#[test]
fn an_api_error_is_one_error_notice_and_no_text_block() {
    let asm = assemble(&[api_error_line(
        1,
        "msg_e",
        "null",
        "API Error: 529 overloaded",
    )]);
    let items = items_of(&asm);
    assert_eq!(
        notices(&items),
        [(
            "notice:msg_e".to_owned(),
            NoticeLevel::Error,
            "API Error: 529 overloaded".to_owned()
        )]
    );
    assert!(texts(&items).is_empty(), "{items:?}");
    assert_eq!(items.len(), 1);
    assert_eq!(asm.unparsed_count(), 0);
}

#[test]
fn the_flag_inside_the_message_counts_like_the_one_on_the_entry() {
    let l = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:01Z","message":{"id":"msg_e","model":"<synthetic>","isApiErrorMessage":true,"content":[{"type":"text","text":"Boom"}]}}"#;
    let asm = assemble(&[l.to_owned()]);
    let items = items_of(&asm);
    assert_eq!(notices(&items).len(), 1);
    assert!(texts(&items).is_empty());
}

#[test]
fn an_api_error_given_as_string_content_is_a_notice_too() {
    let l = r#"{"type":"assistant","uuid":"a1","isApiErrorMessage":true,"timestamp":"2026-01-01T00:00:01Z","message":{"id":"msg_e","model":"<synthetic>","content":"Credit balance is too low"}}"#;
    let items = items_of(&assemble(&[l.to_owned()]));
    assert_eq!(
        notices(&items),
        [(
            "notice:msg_e".to_owned(),
            NoticeLevel::Error,
            "Credit balance is too low".to_owned()
        )]
    );
    assert!(texts(&items).is_empty());
}

#[test]
fn the_text_blocks_of_an_api_error_are_joined_with_newlines() {
    let l = line_with(
        1,
        "msg_e",
        "<synthetic>",
        "null",
        &format!("{},{}", text("first"), text("second")),
        r#""isApiErrorMessage":true,"#,
    );
    let items = items_of(&assemble(&[l]));
    assert_eq!(notices(&items)[0].2, "first\nsecond");
}

#[test]
fn an_api_error_repeated_over_several_lines_gives_one_notice() {
    let lines: Vec<String> = (1..=3)
        .map(|n| api_error_line(n, "msg_e", "null", "API Error: 500"))
        .collect();
    let asm = assemble(&lines);
    let items = items_of(&asm);
    assert_eq!(notices(&items).len(), 1);
    assert_eq!(notices(&items)[0].2, "API Error: 500");
    assert!(texts(&items).is_empty());
    assert_eq!(items.len(), 1);
}

#[test]
fn an_api_error_repeated_with_api_block_indices_gives_one_notice() {
    let lines: Vec<String> = (1..=3)
        .map(|n| {
            line_with(
                n,
                "msg_e",
                "<synthetic>",
                "null",
                &text("API Error: 500"),
                r#""isApiErrorMessage":true,"apiBlockIndex":0,"#,
            )
        })
        .collect();
    let items = items_of(&assemble(&lines));
    assert_eq!(notices(&items).len(), 1);
    assert_eq!(notices(&items)[0].2, "API Error: 500");
}

#[test]
fn a_flagged_message_without_text_has_no_notice() {
    let l = line_with(
        1,
        "msg_e",
        "<synthetic>",
        r#""max_tokens""#,
        "",
        r#""isApiErrorMessage":true,"#,
    );
    let items = items_of(&assemble(&[l]));
    assert!(items.is_empty(), "{items:?}");
}

#[test]
fn an_unflagged_message_with_error_looking_text_stays_text() {
    let items = items_of(&assemble(&[line(
        1,
        "msg_1",
        r#""end_turn""#,
        &text("API Error: not really"),
    )]));
    assert!(notices(&items).is_empty());
    assert_eq!(texts(&items), ["API Error: not really"]);
}

#[test]
fn an_api_error_wins_over_an_unusual_stop_reason() {
    let asm = assemble(&[api_error_line(1, "msg_e", r#""max_tokens""#, "API Error")]);
    let items = items_of(&asm);
    assert_eq!(
        notices(&items),
        [(
            "notice:msg_e".to_owned(),
            NoticeLevel::Error,
            "API Error".to_owned()
        )]
    );
}

#[test]
fn a_synthetic_message_without_the_flag_stays_text() {
    let l = line_with(
        1,
        "msg_s",
        "<synthetic>",
        "null",
        &text("No response requested."),
        "",
    );
    let items = items_of(&assemble(&[l]));
    assert!(notices(&items).is_empty());
    assert!(matches!(
        items.as_slice(),
        [Item::Text { text, .. }] if text == "No response requested."
    ));
}

#[test]
fn a_synthetic_message_does_not_change_the_model_with_or_without_the_flag() {
    let lines = [
        line(1, "msg_1", r#""end_turn""#, &text("Hello.")),
        api_error_line(2, "msg_e", "null", "API Error"),
    ];
    let asm = assemble(&lines);
    let items = items_of(&asm);
    assert!(
        items.iter().all(|i| !matches!(i, Item::ModelChange { .. })),
        "{items:?}"
    );
    assert_eq!(items.len(), 2);
}

// -------------------------------------------------------- stop reasons

#[test]
fn normal_stop_reasons_produce_no_notice() {
    for reason in ["end_turn", "tool_use", "stop_sequence", "pause_turn"] {
        assert!(
            notices_for_stop(&format!("\"{reason}\"")).is_empty(),
            "{reason}"
        );
    }
}

#[test]
fn max_tokens_and_refusal_each_produce_a_warning() {
    for reason in ["max_tokens", "refusal"] {
        assert_eq!(
            notices_for_stop(&format!("\"{reason}\"")),
            [(
                "notice:msg_1".to_owned(),
                NoticeLevel::Warning,
                format!("stopped: {reason}")
            )],
            "{reason}"
        );
    }
}

#[test]
fn any_other_stop_reason_is_a_warning_with_that_reason() {
    assert_eq!(
        notices_for_stop(r#""model_context_window_exceeded""#)[0].2,
        "stopped: model_context_window_exceeded"
    );
}

#[test]
fn the_notice_follows_the_blocks_of_its_message() {
    let items = items_of(&assemble(&[line(
        1,
        "msg_1",
        r#""max_tokens""#,
        &text("Cut off"),
    )]));
    assert!(matches!(items[0], Item::Text { .. }));
    assert!(matches!(items[1], Item::Notice { .. }));
    assert_eq!(items.len(), 2);
}

#[test]
fn a_null_stop_reason_on_every_line_produces_no_notice() {
    let lines = [
        line(1, "msg_1", "null", &text("One")),
        line(2, "msg_1", "null", &tool_use("c1")),
    ];
    let asm = assemble(&lines);
    assert!(notices(&items_of(&asm)).is_empty());
}

#[test]
fn a_missing_stop_reason_produces_no_notice() {
    let l = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:01Z","message":{"id":"msg_1","model":"m","content":[{"type":"text","text":"Hi"}]}}"#;
    assert!(notices(&items_of(&assemble(&[l.to_owned()]))).is_empty());
}

#[test]
fn null_on_the_first_lines_and_max_tokens_on_a_later_one_gives_one_notice() {
    let lines = [
        line(1, "msg_1", "null", &text("One")),
        line(2, "msg_1", "null", &tool_use("c1")),
        line(3, "msg_1", r#""max_tokens""#, &text("Two")),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(
        notices(&items),
        [(
            "notice:msg_1".to_owned(),
            NoticeLevel::Warning,
            "stopped: max_tokens".to_owned()
        )]
    );
}

#[test]
fn a_later_null_does_not_undo_an_earlier_stop_reason() {
    let lines = [
        line(1, "msg_1", r#""max_tokens""#, &text("One")),
        line(2, "msg_1", "null", &tool_use("c1")),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(notices(&items).len(), 1);
}

#[test]
fn the_last_stop_reason_decides_and_a_normal_one_removes_the_notice() {
    let lines = [
        line(1, "msg_1", r#""max_tokens""#, &text("One")),
        line(2, "msg_1", r#""end_turn""#, &text("Two")),
    ];
    let items = items_of(&assemble(&lines));
    assert!(notices(&items).is_empty(), "{items:?}");
    assert_eq!(items.len(), 2);
}

#[test]
fn a_tool_result_after_the_notice_was_removed_still_attaches_to_its_call() {
    // The notice sits before the call (`[text, notice, call]`); taking it out
    // shifts the call, which a result arriving afterwards must still find.
    let lines = [
        line(1, "msg_1", r#""max_tokens""#, &text("One")),
        line(2, "msg_1", "null", &tool_use("c1")),
        line(3, "msg_1", r#""end_turn""#, &text("Two")),
        result_line(4, "c1"),
    ];
    let asm = assemble(&lines);
    let items = items_of(&asm);
    assert!(notices(&items).is_empty(), "{items:?}");
    let call = items
        .iter()
        .find_map(|i| match i {
            Item::ToolCall(c) if c.id.as_str() == "c1" => Some(c),
            _ => None,
        })
        .expect("the call is still there");
    assert_eq!(call.status, ToolStatus::Ok);
    assert_eq!(
        call.result.as_ref().unwrap().output.text().unwrap(),
        "file.txt"
    );
}

#[test]
fn a_removed_notice_keeps_every_call_after_it_attachable() {
    // Two calls after the notice, a result for each, and the removal between
    // them.
    let lines = [
        line(1, "msg_1", r#""max_tokens""#, &text("One")),
        line(2, "msg_1", "null", &tool_use("c1")),
        line(3, "msg_2", "null", &tool_use("c2")),
        line(4, "msg_1", r#""end_turn""#, &text("Two")),
        result_line(5, "c2"),
        result_line(6, "c1"),
    ];
    let items = items_of(&assemble(&lines));
    assert!(notices(&items).is_empty());
    for id in ["c1", "c2"] {
        let status = items.iter().find_map(|i| match i {
            Item::ToolCall(c) if c.id.as_str() == id => Some(c.status),
            _ => None,
        });
        assert_eq!(status, Some(ToolStatus::Ok), "{id}");
    }
}

#[test]
fn a_tool_result_still_attaches_after_a_notice_is_added() {
    let lines = [
        line(1, "msg_1", "null", &tool_use("c1")),
        line(2, "msg_2", r#""refusal""#, &text("No")),
        result_line(3, "c1"),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(notices(&items).len(), 1);
    let status = items.iter().find_map(|i| match i {
        Item::ToolCall(c) => Some(c.status),
        _ => None,
    });
    assert_eq!(status, Some(ToolStatus::Ok));
}

#[test]
fn a_notice_is_upserted_not_added_again() {
    let lines = [
        line(1, "msg_1", r#""max_tokens""#, &text("One")),
        line(2, "msg_1", r#""max_tokens""#, &text("One more")),
        line(3, "msg_1", r#""refusal""#, &text("Two")),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(
        notices(&items),
        [(
            "notice:msg_1".to_owned(),
            NoticeLevel::Warning,
            "stopped: refusal".to_owned()
        )]
    );
}

#[test]
fn each_message_has_its_own_notice() {
    let lines = [
        line(1, "msg_1", r#""max_tokens""#, &text("One")),
        line(2, "msg_2", r#""refusal""#, &text("Two")),
    ];
    let ids: Vec<String> = notices(&items_of(&assemble(&lines)))
        .into_iter()
        .map(|n| n.0)
        .collect();
    assert_eq!(ids, ["notice:msg_1", "notice:msg_2"]);
}

#[test]
fn distinct_error_blocks_on_separate_lines_without_an_index_are_joined() {
    let lines = [
        api_error_line(1, "msg_e", "null", "first"),
        api_error_line(2, "msg_e", "null", "second"),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(
        notices(&items),
        [(
            "notice:msg_e".to_owned(),
            NoticeLevel::Error,
            "first\nsecond".to_owned()
        )]
    );
    assert!(texts(&items).is_empty());
}

#[test]
fn repeated_lines_of_several_error_blocks_without_an_index_collapse() {
    let lines: Vec<String> = (0..2)
        .flat_map(|round| {
            [
                api_error_line(1 + 2 * round, "msg_e", "null", "first"),
                api_error_line(2 + 2 * round, "msg_e", "null", "second"),
            ]
        })
        .collect();
    let items = items_of(&assemble(&lines));
    assert_eq!(notices(&items)[0].2, "first\nsecond");
    assert_eq!(items.len(), 1);
}

#[test]
fn a_grown_error_block_without_an_index_replaces_the_earlier_one() {
    let lines = [
        api_error_line(1, "msg_e", "null", "API Error"),
        api_error_line(2, "msg_e", "null", "API Error: 529"),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(notices(&items)[0].2, "API Error: 529");
}

#[test]
fn a_flag_arriving_after_the_same_text_replaces_the_text_block() {
    let lines = [
        line(1, "msg_e", "null", &text("API Error")),
        api_error_line(2, "msg_e", "null", "API Error"),
    ];
    let asm = assemble(&lines);
    let items = items_of(&asm);
    assert_eq!(
        notices(&items),
        [(
            "notice:msg_e".to_owned(),
            NoticeLevel::Error,
            "API Error".to_owned()
        )]
    );
    assert!(texts(&items).is_empty(), "{items:?}");
    assert_eq!(items.len(), 1);
}

#[test]
fn text_shown_before_the_flag_joins_the_errors_text() {
    let lines = [
        line(1, "msg_e", "null", &text("first")),
        api_error_line(2, "msg_e", "null", "second"),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(notices(&items)[0].2, "first\nsecond");
    assert!(texts(&items).is_empty());
}

#[test]
fn other_messages_text_stays_when_a_message_turns_out_to_be_an_error() {
    let lines = [
        line(1, "msg_1", r#""end_turn""#, &text("Real answer")),
        line(2, "msg_e", "null", &text("API Error")),
        api_error_line(3, "msg_e", "null", "API Error"),
    ];
    let items = items_of(&assemble(&lines));
    assert_eq!(texts(&items), ["Real answer"]);
    assert_eq!(notices(&items).len(), 1);
}

#[test]
fn a_tool_result_attaches_to_a_call_that_follows_the_text_taken_into_the_error() {
    let lines = [
        line(1, "msg_e", "null", &text("API Error")),
        line(2, "msg_1", "null", &tool_use("c1")),
        api_error_line(3, "msg_e", "null", "API Error"),
        result_line(4, "c1"),
    ];
    let items = items_of(&assemble(&lines));
    assert!(texts(&items).is_empty(), "{items:?}");
    let status = items.iter().find_map(|i| match i {
        Item::ToolCall(c) => Some(c.status),
        _ => None,
    });
    assert_eq!(status, Some(ToolStatus::Ok));
}

#[test]
fn an_empty_stop_reason_is_no_reason() {
    assert!(notices_for_stop(r#""""#).is_empty());
}

// ------------------------------------------------- unexpected JSON types

#[test]
fn stop_reason_and_flag_of_unexpected_types_leave_the_line_parsed_without_notice() {
    for (stop, flag) in [
        ("5", r#""isApiErrorMessage":"yes","#),
        ("true", r#""isApiErrorMessage":1,"#),
        (r#"["max_tokens"]"#, r#""isApiErrorMessage":{},"#),
        (r#"{"a":1}"#, r#""isApiErrorMessage":[true],"#),
        ("null", r#""isApiErrorMessage":null,"#),
    ] {
        let l = line_with(1, "msg_1", "m", stop, &text("Fine."), flag);
        let asm = assemble(&[l]);
        let items = items_of(&asm);
        assert_eq!(asm.unparsed_count(), 0, "{stop} {flag}");
        assert!(notices(&items).is_empty(), "{stop} {flag}");
        assert_eq!(texts(&items), ["Fine."], "{stop} {flag}");
    }
}

#[test]
fn the_flag_of_an_unexpected_type_inside_the_message_leaves_the_line_parsed() {
    let l = r#"{"type":"assistant","uuid":"a1","timestamp":"2026-01-01T00:00:01Z","message":{"id":"msg_1","model":"m","stop_reason":7,"isApiErrorMessage":"no","content":[{"type":"text","text":"Fine."}]}}"#;
    let asm = assemble(&[l.to_owned()]);
    assert_eq!(asm.unparsed_count(), 0);
    let items = items_of(&asm);
    assert!(notices(&items).is_empty());
    assert_eq!(texts(&items), ["Fine."]);
}

#[test]
fn invalid_utf8_in_a_top_level_flag_keeps_the_line_parsed() {
    // Bytes that are not UTF-8 inside a field that is not a boolean: the line
    // is parsed, as it is when the field is unknown, with no notice.
    let mut bytes = PROMPT.as_bytes().to_vec();
    bytes.extend_from_slice(
        b"\n{\"type\":\"assistant\",\"uuid\":\"a1\",\"isApiErrorMessage\":\"x\xffy\",\"timestamp\":\"2026-01-01T00:00:01Z\",\"message\":{\"id\":\"msg_1\",\"model\":\"m\",\"stop_reason\":null,\"content\":[{\"type\":\"text\",\"text\":\"Fine.\"}]}}\n",
    );
    let asm = Assembler::from_memory(Arc::from(bytes));
    assert_eq!(asm.unparsed_count(), 0);
    let items = items_of(&asm);
    assert!(notices(&items).is_empty());
    assert_eq!(texts(&items), ["Fine."]);
}

// ----------------------------------------------------------- chunking

/// A transcript with every kind of notice, a removal and a late result.
fn busy_transcript() -> Arc<[u8]> {
    let lines = [
        line(1, "msg_1", r#""max_tokens""#, &text("Cut")),
        line(2, "msg_1", "null", &tool_use("c1")),
        line(3, "msg_1", r#""end_turn""#, &text("Done")),
        result_line(4, "c1"),
        api_error_line(5, "msg_e", "null", "API Error: 529 overloaded"),
        api_error_line(6, "msg_e", "null", "API Error: 529 overloaded"),
        line(7, "msg_2", "null", &text("Part")),
        line(8, "msg_2", r#""refusal""#, &text("Refused")),
        line_with(
            9,
            "msg_s",
            "<synthetic>",
            "null",
            &text("No response requested."),
            "",
        ),
    ];
    Arc::from(transcript(&lines).as_bytes())
}

struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn any_chunking_gives_the_same_final_turns_with_notices() {
    let bytes = busy_transcript();
    let whole = Assembler::from_memory(Arc::clone(&bytes)).into_turns();
    let items = match &*whole[1] {
        Turn::Assistant(t) => t.items.clone(),
        other => panic!("{other:?}"),
    };
    let found = notices(&items);
    assert_eq!(
        found.iter().map(|n| n.0.as_str()).collect::<Vec<_>>(),
        ["notice:msg_e", "notice:msg_2"]
    );

    for seed in 1..=60u64 {
        let mut rng = XorShift(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
        let mut asm = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
        let mut replayed: Vec<Arc<Turn>> = Vec::new();
        let mut pos = 0;
        while pos < bytes.len() {
            let end = (pos + 1 + (rng.next() % 300) as usize).min(bytes.len());
            for update in asm.push_chunk(&bytes[pos..end]) {
                update.apply(&mut replayed);
            }
            pos = end;
        }
        for update in asm.finish() {
            update.apply(&mut replayed);
        }
        assert_eq!(replayed, asm.turns(), "seed {seed}: replayed updates");
        assert_eq!(asm.into_turns(), whole, "seed {seed}");
    }
}

#[test]
fn line_by_line_and_byte_by_byte_give_the_same_final_turns_with_notices() {
    let bytes = busy_transcript();
    let whole = Assembler::from_memory(Arc::clone(&bytes)).into_turns();

    let mut by_lines = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
    let mut start = 0u64;
    for l in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        by_lines.push_line(l, start..start + l.len() as u64);
        start += l.len() as u64 + 1;
    }
    assert_eq!(by_lines.into_turns(), whole);

    let mut bytewise = Assembler::new(ByteSource::Memory(Arc::clone(&bytes)));
    let mut replayed: Vec<Arc<Turn>> = Vec::new();
    for b in bytes.chunks(1) {
        for update in bytewise.push_chunk(b) {
            update.apply(&mut replayed);
        }
    }
    for update in bytewise.finish() {
        update.apply(&mut replayed);
    }
    assert_eq!(replayed, whole);
}

fn feed(asm: &mut Assembler, replayed: &mut Vec<Arc<Turn>>, l: &str) {
    for update in asm.push_chunk(format!("{l}\n").as_bytes()) {
        update.apply(replayed);
    }
}

#[test]
fn removing_a_notice_is_reported_as_an_update_of_its_turn() {
    let mut asm = Assembler::new(ByteSource::Memory(Arc::from(&b""[..])));
    let mut replayed: Vec<Arc<Turn>> = Vec::new();
    feed(&mut asm, &mut replayed, PROMPT);
    let first = line(1, "msg_1", r#""max_tokens""#, &text("One"));
    feed(&mut asm, &mut replayed, &first);
    assert!(matches!(&*replayed[1], Turn::Assistant(t) if notices(&t.items).len() == 1));
    let second = line(2, "msg_1", r#""end_turn""#, &text("One"));
    feed(&mut asm, &mut replayed, &second);
    assert!(matches!(&*replayed[1], Turn::Assistant(t) if notices(&t.items).is_empty()));
}
