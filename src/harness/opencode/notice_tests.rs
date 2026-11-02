//! Notices for abnormal endings of assistant rows (cycle 2 SPEC §4.2).

use std::sync::Arc;

use super::assemble::{Assembler, Row};
use crate::model::{Item, NoticeLevel, Turn};

fn row(id: &str, kind: &str, seq: i64, data: &str) -> Row {
    Row {
        id: id.to_owned(),
        kind: kind.to_owned(),
        seq,
        created: 1_000 * seq,
        updated: 1_000 * seq,
        data: data.to_owned(),
    }
}

fn user(id: &str, seq: i64) -> Row {
    row(id, "user", seq, r#"{"text":"go"}"#)
}

/// An assistant row with one text part and `extra` as further top-level keys
/// (`"finish":"error"`, say).
fn assistant(id: &str, seq: i64, extra: &str) -> Row {
    let extra = if extra.is_empty() {
        String::new()
    } else {
        format!(",{extra}")
    };
    row(
        id,
        "assistant",
        seq,
        &format!(
            r#"{{"model":{{"id":"m"}},"content":[{{"type":"text","text":"hello {id}"}}]{extra}}}"#
        ),
    )
}

/// An assistant row with a tool part in state `status`.
fn tool_row(id: &str, seq: i64, status: &str, extra: &str) -> Row {
    let extra = if extra.is_empty() {
        String::new()
    } else {
        format!(",{extra}")
    };
    let content = if status == "completed" {
        r#","content":[{"type":"text","text":"done"}]"#
    } else {
        ""
    };
    row(
        id,
        "assistant",
        seq,
        &format!(
            r#"{{"model":{{"id":"m"}},"content":[{{"type":"tool","id":"call-{id}","name":"shell","state":{{"status":"{status}","input":{{"command":"ls"}}{content}}}}}]{extra}}}"#
        ),
    )
}

fn assemble(rows: &[Row]) -> Assembler {
    let mut asm = Assembler::new();
    for r in rows {
        asm.push_row(r);
    }
    asm.finish();
    asm
}

/// The items of the Assistant turn at `index`.
fn items(asm: &Assembler, index: usize) -> Vec<Item> {
    let turn: &Arc<Turn> = &asm.turns()[index];
    match &**turn {
        Turn::Assistant(a) => a.items.clone(),
        other => panic!("not an assistant turn: {other:?}"),
    }
}

fn notices(asm: &Assembler) -> Vec<(String, NoticeLevel, String)> {
    items(asm, 1)
        .into_iter()
        .filter_map(|item| match item {
            Item::Notice { id, level, text } => Some((id.to_string(), level, text)),
            _ => None,
        })
        .collect()
}

fn ids(asm: &Assembler) -> Vec<String> {
    items(asm, 1).iter().map(|i| i.id().to_string()).collect()
}

/// The notice of a single row with the given extra keys.
fn notice_of(extra: &str) -> Option<(NoticeLevel, String)> {
    let asm = assemble(&[user("u1", 1), assistant("a1", 2, extra)]);
    let found = notices(&asm);
    assert!(found.len() <= 1, "{found:?}");
    found.into_iter().next().map(|(id, level, text)| {
        assert_eq!(id, "notice:a1");
        (level, text)
    })
}

#[test]
fn an_aborted_row_is_a_warning_with_the_message_when_there_is_one() {
    assert_eq!(
        notice_of(r#""finish":"error","error":{"type":"aborted"}"#),
        Some((NoticeLevel::Warning, "aborted".to_owned()))
    );
    assert_eq!(
        notice_of(r#""finish":"error","error":{"type":"aborted","message":"Step interrupted"}"#),
        Some((NoticeLevel::Warning, "aborted: Step interrupted".to_owned()))
    );
}

#[test]
fn another_error_type_is_an_error_with_whichever_parts_exist() {
    assert_eq!(
        notice_of(
            r#""finish":"error","error":{"type":"provider.transport","message":"connection reset"}"#
        ),
        Some((
            NoticeLevel::Error,
            "provider.transport: connection reset".to_owned()
        ))
    );
    assert_eq!(
        notice_of(r#""finish":"error","error":{"type":"provider.transport"}"#),
        Some((NoticeLevel::Error, "provider.transport".to_owned()))
    );
    assert_eq!(
        notice_of(r#""finish":"error","error":{"message":"it broke"}"#),
        Some((NoticeLevel::Error, "it broke".to_owned()))
    );
}

#[test]
fn an_error_without_type_or_message_says_error() {
    for extra in [
        r#""finish":"error""#,
        r#""finish":"error","error":{}"#,
        r#""finish":"error","error":{"type":"  ","message":""}"#,
    ] {
        assert_eq!(
            notice_of(extra),
            Some((NoticeLevel::Error, "error".to_owned())),
            "{extra}"
        );
    }
}

#[test]
fn another_finish_is_a_warning_naming_it() {
    assert_eq!(
        notice_of(r#""finish":"length""#),
        Some((NoticeLevel::Warning, "stopped: length".to_owned()))
    );
    assert_eq!(
        notice_of(r#""finish":"content-filter""#),
        Some((NoticeLevel::Warning, "stopped: content-filter".to_owned()))
    );
}

#[test]
fn a_normal_ending_produces_no_notice() {
    for extra in [
        r#""finish":"stop""#,
        r#""finish":"tool-calls""#,
        r#""finish":"stop","error":{"type":"aborted"}"#,
        r#""finish":"""#,
        r#""finish":null"#,
        "",
        // An error without `finish: "error"` is not an abnormal ending.
        r#""error":{"type":"aborted","message":"x"}"#,
    ] {
        assert_eq!(notice_of(extra), None, "{extra}");
    }
}

#[test]
fn the_notice_follows_the_parts_of_its_row() {
    let asm = assemble(&[
        user("u1", 1),
        tool_row(
            "a1",
            2,
            "completed",
            r#""finish":"error","error":{"type":"aborted"}"#,
        ),
        assistant("a2", 3, ""),
    ]);
    assert_eq!(ids(&asm), ["call-a1", "notice:a1", "a2:0"]);
}

#[test]
fn a_row_updated_in_place_has_one_notice_with_the_new_text() {
    let mut asm = assemble(&[
        user("u1", 1),
        assistant(
            "a1",
            2,
            r#""finish":"error","error":{"type":"aborted","message":"first"}"#,
        ),
        assistant("a2", 3, ""),
    ]);
    asm.replace_row(&assistant(
        "a1",
        2,
        r#""finish":"error","error":{"type":"aborted","message":"second"}"#,
    ));
    assert_eq!(
        notices(&asm),
        [(
            "notice:a1".to_owned(),
            NoticeLevel::Warning,
            "aborted: second".to_owned()
        )]
    );
    // It keeps its place between the row's text and the next row's.
    assert_eq!(ids(&asm), ["a1:0", "notice:a1", "a2:0"]);
}

#[test]
fn a_streaming_row_that_ends_in_error_gains_its_notice() {
    let mut asm = assemble(&[
        user("u1", 1),
        tool_row("a1", 2, "pending", ""),
        tool_row("a2", 3, "pending", ""),
    ]);
    assert_eq!(notices(&asm), []);
    asm.replace_row(&tool_row(
        "a2",
        3,
        "completed",
        r#""finish":"error","error":{"type":"provider.transport","message":"reset"}"#,
    ));
    assert_eq!(
        notices(&asm),
        [(
            "notice:a2".to_owned(),
            NoticeLevel::Error,
            "provider.transport: reset".to_owned()
        )]
    );
    assert_eq!(ids(&asm), ["call-a1", "call-a2", "notice:a2"]);
    // The row still counts once in the stats.
    let Turn::Assistant(turn) = &*asm.turns()[1] else {
        panic!("an assistant turn")
    };
    assert_eq!(turn.stats.as_ref().unwrap().message_count, 2);
}

#[test]
fn a_row_that_ends_normally_after_an_error_loses_its_notice() {
    let mut asm = assemble(&[
        user("u1", 1),
        assistant("a1", 2, r#""finish":"error","error":{"type":"aborted"}"#),
        tool_row("a2", 3, "pending", ""),
        user("u2", 4),
        assistant("a3", 5, ""),
    ]);
    assert_eq!(ids(&asm), ["a1:0", "notice:a1", "call-a2"]);
    asm.replace_row(&assistant("a1", 2, r#""finish":"stop""#));
    assert_eq!(notices(&asm), []);
    assert_eq!(ids(&asm), ["a1:0", "call-a2"]);

    // The items after the removed one still receive their updates, in the
    // turn that is no longer the current one, and the notice can return.
    asm.replace_row(&tool_row("a2", 3, "completed", ""));
    let Item::ToolCall(call) = &items(&asm, 1)[1] else {
        panic!("the tool call of a2 is the second item")
    };
    assert_eq!(call.id.as_str(), "call-a2");
    assert_eq!(call.status, crate::model::ToolStatus::Ok);
    assert_eq!(ids(&asm), ["a1:0", "call-a2"]);
    asm.replace_row(&assistant("a1", 2, r#""finish":"length""#));
    assert_eq!(ids(&asm), ["a1:0", "call-a2", "notice:a1"]);
}

#[test]
fn later_rows_of_the_current_turn_land_after_a_removed_notice() {
    let mut asm = Assembler::new();
    asm.push_row(&user("u1", 1));
    asm.push_row(&assistant("a1", 2, r#""finish":"length""#));
    asm.push_row(&tool_row("a2", 3, "pending", ""));
    asm.replace_row(&assistant("a1", 2, r#""finish":"stop""#));
    asm.replace_row(&tool_row("a2", 3, "completed", ""));
    asm.push_row(&assistant("a3", 4, ""));
    asm.finish();
    assert_eq!(ids(&asm), ["a1:0", "call-a2", "a3:0"]);
    let Item::ToolCall(call) = &items(&asm, 1)[1] else {
        panic!("a tool call")
    };
    assert_eq!(call.status, crate::model::ToolStatus::Ok);
}

#[test]
fn an_error_of_an_unexpected_json_type_leaves_the_row_parsed() {
    for error in [
        "5",
        "true",
        "null",
        r#"["aborted"]"#,
        r#"{"type":5,"message":{"a":1}}"#,
        r#"{"type":"a","type":"b"}"#,
    ] {
        let asm = assemble(&[
            user("u1", 1),
            assistant("a1", 2, &format!(r#""finish":"error","error":{error}"#)),
        ]);
        assert_eq!(asm.unparsed_count(), 0, "{error}");
        assert_eq!(
            ids(&asm),
            ["a1:0", "notice:a1"],
            "the row is parsed, {error}"
        );
        assert_eq!(notices(&asm)[0].2, "error", "no usable parts, {error}");
    }
}

#[test]
fn a_string_error_counts_as_its_message() {
    assert_eq!(
        notice_of(r#""finish":"error","error":"it broke""#),
        Some((NoticeLevel::Error, "it broke".to_owned()))
    );
}

#[test]
fn a_finish_of_an_unexpected_json_type_counts_as_absent() {
    for finish in ["5", "true", "[]", r#"{"a":1}"#, "99999999999999999999999"] {
        let asm = assemble(&[
            user("u1", 1),
            assistant("a1", 2, &format!(r#""finish":{finish}"#)),
        ]);
        assert_eq!(asm.unparsed_count(), 0, "{finish}");
        assert_eq!(ids(&asm), ["a1:0"], "{finish}");
    }
}

#[test]
fn a_notice_does_not_change_the_turn_stats() {
    let plain = assemble(&[user("u1", 1), assistant("a1", 2, "")]);
    let failed = assemble(&[
        user("u1", 1),
        assistant("a1", 2, r#""finish":"error","error":{"type":"aborted"}"#),
    ]);
    let stats = |asm: &Assembler| match &*asm.turns()[1] {
        Turn::Assistant(a) => a.stats.clone(),
        _ => panic!("an assistant turn"),
    };
    assert_eq!(stats(&plain), stats(&failed));
}
