//! The details overlay's rows read from OpenCode's rows (cycle 2 SPEC §5.2,
//! §5.3): each group of the table, a row without the fields, an unreadable
//! source and a database that stays as it was.

use std::sync::Arc;

use super::assemble::{Assembler, Row};
use super::db::{Db, OpenMode};
use super::details::{read_groups, section_details};
use super::testing::TempDb;
use crate::components::turn_block::fixtures::formatter;
use crate::components::turn_block::{SectionId, SectionKey};
use crate::components::{
    DetailsContent, compose, section_details as model_details, section_source,
};
use crate::model::{
    Item, ItemId, NoticeLevel, SectionRefs, SourceGroup, SourceGroupKind, SourceKey, SourceKind,
    SourceValue, Turn,
};

const SESSION: &str = "ses_details";

// -- a database with one session of the rows a test gives it

struct Fx {
    db: TempDb,
}

fn sql(text: &str) -> String {
    text.replace('\'', "''")
}

impl Fx {
    /// The fixture database with a session `ses_details` holding `rows`
    /// (`id`, `type`, `data`), in this order.
    fn new(rows: &[(&str, &str, &str)]) -> Self {
        let db = TempDb::new();
        db.exec(&format!(
            "INSERT INTO session_v2 (id, project_id, slug, directory, version, time_created, time_updated) \
             VALUES ('{SESSION}', 'prj_alpha', '{SESSION}', '/home/user/alpha', '1.0.0', 1, 1);"
        ));
        let fx = Self { db };
        for (seq, (id, kind, data)) in (1..).zip(rows) {
            fx.insert(id, kind, seq, data);
        }
        fx
    }

    fn insert(&self, id: &str, kind: &str, seq: i64, data: &str) {
        self.db.exec(&format!(
            "INSERT INTO session_message (id, session_id, type, seq, time_created, time_updated, data) \
             VALUES ('{id}', '{SESSION}', '{kind}', {seq}, {t}, {t}, '{data}');",
            id = sql(id),
            kind = sql(kind),
            t = 1_700_000_000_000_i64 + seq,
            data = sql(data),
        ));
    }

    /// Replaces the `data` of row `id`.
    fn set_data(&self, id: &str, data: &str) {
        self.db.exec(&format!(
            "UPDATE session_message SET data = '{}' WHERE id = '{}';",
            sql(data),
            sql(id)
        ));
    }

    fn delete(&self, id: &str) {
        self.db.exec(&format!(
            "DELETE FROM session_message WHERE id = '{}';",
            sql(id)
        ));
    }

    /// The turns the assembler makes of the session's rows.
    fn turns(&self) -> Vec<Arc<Turn>> {
        let db = Db::open(&self.db.path).unwrap();
        let rows = db
            .select(
                "SELECT id, type, seq, time_created, time_updated, data FROM session_message \
                 WHERE session_id = ?1 ORDER BY seq",
                [SESSION],
                |row| {
                    Ok(Row {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        seq: row.get(2)?,
                        created: row.get(3)?,
                        updated: row.get(4)?,
                        data: row.get(5)?,
                    })
                },
            )
            .unwrap();
        let mut asm = Assembler::new();
        for row in &rows {
            asm.push_row(row);
        }
        asm.into_turns()
    }

    /// The overlay's content for the section `key` of turn `turn`: what the
    /// model knows, composed with what the adapter reads.
    fn content(&self, turn: usize, key: SectionKey) -> DetailsContent {
        self.content_of(&self.turns(), turn, key)
    }

    /// [`content`](Self::content) for turns assembled earlier, so that the
    /// database can have changed since.
    fn content_of(&self, turns: &[Arc<Turn>], turn: usize, key: SectionKey) -> DetailsContent {
        let turn = &turns[turn];
        let id = SectionId {
            turn: turn.id().clone(),
            key,
        };
        let model = model_details_of(turn, &id);
        let read = section_source(turn, &id)
            .map(|source| section_details(&self.db.path, &source))
            .unwrap_or_default();
        compose(model, &read, &formatter())
    }

    fn item(&self, turn: usize, id: &str) -> DetailsContent {
        self.content(turn, SectionKey::Item(ItemId::from(id)))
    }

    /// The groups read for the section `key` of turn `turn`.
    fn read(&self, turn: usize, key: SourceKey) -> Vec<SourceGroup> {
        let turns = self.turns();
        let refs = turns[turn].sources().get(&key).expect("references");
        let kind = match key {
            SourceKey::Own => SourceKind::Prompt,
            SourceKey::Footer => SourceKind::Other,
            SourceKey::Item(_) => SourceKind::Message,
        };
        section_details(&self.db.path, &crate::model::SectionSource { kind, refs })
    }
}

fn model_details_of(turn: &Turn, id: &SectionId) -> DetailsContent {
    model_details(turn, id, &formatter()).expect("a section")
}

/// `(label, value)` of the rows of group `title`; `None` without the group.
fn rows<'a>(content: &'a DetailsContent, title: &str) -> Option<Vec<(&'a str, &'a str)>> {
    content.groups().iter().find(|g| g.title == title).map(|g| {
        g.rows
            .iter()
            .map(|r| (r.label.as_str(), r.value.as_str()))
            .collect()
    })
}

fn labels<'a>(content: &'a DetailsContent, title: &str) -> Vec<&'a str> {
    rows(content, title)
        .unwrap_or_default()
        .into_iter()
        .map(|(label, _)| label)
        .collect()
}

fn titles(content: &DetailsContent) -> Vec<&str> {
    content.groups().iter().map(|g| g.title.as_str()).collect()
}

fn has_unavailable(content: &DetailsContent, title: &str) -> bool {
    rows(content, title).is_some_and(|rows| rows.contains(&("unavailable", "")))
}

const USER: &str = r#"{"time":{"created":1700000000000},"text":"Hello"}"#;

/// An assistant row with everything the overlay can show.
const FULL: &str = r#"{"time":{"created":1700000001000,"streamed":1700000003000,"completed":1700000004000},"agent":"build","model":{"id":"acme/coder-1","providerID":"acme","variant":"high"},"content":[{"type":"text","text":"Hello there"},{"type":"reasoning","text":"Thinking about it"},{"type":"tool","id":"call-1","name":"shell","state":{"status":"completed","input":{"command":"ls"},"content":[{"type":"text","text":"a\nb\n"}]},"time":{"created":1700000001500,"ran":1700000001600,"completed":1700000002500}}],"finish":"error","rawFinish":"content_filter","error":{"type":"api","message":"overloaded"},"cost":0.25,"tokens":{"input":1000,"output":200},"snapshot":{"start":"aaa111","end":"bbb222"}}"#;

fn full() -> Fx {
    Fx::new(&[("u1", "user", USER), ("a1", "assistant", FULL)])
}

// -- the groups

#[test]
fn a_text_section_has_the_turn_message_and_environment_groups() {
    let content = full().item(1, "a1:0");
    assert_eq!(titles(&content), ["Turn", "Message", "Environment"]);
    assert_eq!(
        rows(&content, "Message").unwrap(),
        [
            ("row id", "a1"),
            ("time.created", "2023-11-14 22:13:21Z"),
            ("time.streamed", "2023-11-14 22:13:23Z"),
            ("time.completed", "2023-11-14 22:13:24Z"),
            ("finish", "error"),
            ("rawFinish", "content_filter"),
            ("error", "api: overloaded"),
        ]
    );
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [("snapshot.start", "aaa111"), ("snapshot.end", "bbb222")]
    );
}

#[test]
fn thinking_and_notice_sections_have_the_message_group_of_their_row() {
    let fx = full();
    for id in ["a1:1", "notice:a1"] {
        let content = fx.item(1, id);
        assert_eq!(
            rows(&content, "Message").unwrap()[0],
            ("row id", "a1"),
            "{id}"
        );
        assert_eq!(
            labels(&content, "Environment"),
            ["snapshot.start", "snapshot.end"]
        );
    }
}

#[test]
fn the_turn_group_has_cost_then_provider_then_agent() {
    let content = full().item(1, "a1:0");
    let turn = rows(&content, "Turn").unwrap();
    let labels: Vec<&str> = turn.iter().map(|(l, _)| *l).collect();
    let at = |label: &str| labels.iter().position(|l| *l == label).unwrap();
    assert_eq!(turn[at("cost")], ("cost", "$0.25"));
    assert_eq!(turn[at("provider")], ("provider", "acme"));
    assert_eq!(turn[at("agent")], ("agent", "build"));
    assert!(at("cost") < at("provider") && at("provider") < at("agent"));
    assert_eq!(at("agent"), labels.len() - 1, "{labels:?}");
}

#[test]
fn a_turn_without_cost_provider_and_agent_leaves_them_out() {
    let fx = Fx::new(&[
        ("u1", "user", USER),
        (
            "a1",
            "assistant",
            r#"{"time":{"created":1700000001000},"model":{"id":"m"},"content":[{"type":"text","text":"hi"}]}"#,
        ),
    ]);
    let content = fx.item(1, "a1:0");
    let labels = labels(&content, "Turn");
    for label in ["cost", "provider", "agent"] {
        assert!(!labels.contains(&label), "{label} in {labels:?}");
    }
    assert!(!has_unavailable(&content, "Turn"));
}

#[test]
fn provider_and_agent_come_from_the_first_assistant_row_of_the_turn() {
    let first = r#"{"agent":"build","model":{"id":"m","providerID":"first"},"content":[{"type":"text","text":"one"}]}"#;
    let second = r#"{"agent":"plan","model":{"id":"m","providerID":"second"},"content":[{"type":"text","text":"two"}]}"#;
    let fx = Fx::new(&[
        ("u1", "user", USER),
        ("a1", "assistant", first),
        ("a2", "assistant", second),
    ]);
    // The second row's text is in the same turn.
    let content = fx.item(1, "a2:0");
    let turn = rows(&content, "Turn").unwrap();
    assert!(turn.contains(&("provider", "first")), "{turn:?}");
    assert!(turn.contains(&("agent", "build")), "{turn:?}");
    assert_eq!(rows(&content, "Message").unwrap()[0], ("row id", "a2"));
}

#[test]
fn a_tool_call_has_start_then_duration_then_status() {
    let content = full().item(1, "call-1");
    assert_eq!(titles(&content), ["Turn", "Tool call", "Environment"]);
    assert_eq!(
        rows(&content, "Tool call").unwrap(),
        [
            ("start", "2023-11-14 22:13:21Z"),
            ("duration", "1s"),
            ("status", "ok"),
        ]
    );
    assert_eq!(
        labels(&content, "Environment"),
        ["snapshot.start", "snapshot.end"]
    );
}

#[test]
fn a_tool_call_and_its_result_share_one_reference() {
    let fx = full();
    let turns = fx.turns();
    let refs = turns[1]
        .sources()
        .get(&SourceKey::Item(ItemId::from("call-1")))
        .unwrap();
    let SectionRefs::Rows(refs) = refs else {
        panic!("a row reference: {refs:?}")
    };
    let section = refs.section.unwrap();
    assert_eq!(
        (section.row.as_str(), section.part.as_deref()),
        ("a1", Some("call-1"))
    );
    assert_eq!(refs.turn.as_deref(), Some("a1"));
    // A text part is the row alone.
    let text = turns[1]
        .sources()
        .get(&SourceKey::Item(ItemId::from("a1:0")))
        .unwrap();
    let section = text.rows().unwrap().section.clone().unwrap();
    assert_eq!((section.row.as_str(), section.part), ("a1", None));
    // No second reference for the result: the call's section has no lines.
    assert!(text.lines().is_none());
    let call = turns[1]
        .sources()
        .get(&SourceKey::Item(ItemId::from("call-1")))
        .unwrap();
    assert!(call.lines().is_none());
}

#[test]
fn a_tool_part_without_an_id_is_found_by_its_position() {
    let data = r#"{"content":[{"type":"text","text":"x"},{"type":"tool","name":"shell","state":{"status":"completed","input":{},"content":[]},"time":{"created":1700000007000,"completed":1700000008000}}]}"#;
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", data)]);
    let content = fx.item(1, "a1:1");
    assert_eq!(
        rows(&content, "Tool call").unwrap()[0],
        ("start", "2023-11-14 22:13:27Z")
    );
}

#[test]
fn a_prompt_shows_the_time_created_of_its_row_once_in_place_of_the_timestamp() {
    // The column says 1_700_000_000_001; `data` says another time.
    let fx = Fx::new(&[(
        "u1",
        "user",
        r#"{"time":{"created":1700000100000},"text":"Hello"}"#,
    )]);
    let content = fx.content(0, SectionKey::Prompt);
    assert_eq!(titles(&content), ["Prompt"]);
    assert_eq!(
        rows(&content, "Prompt").unwrap(),
        [("time.created", "2023-11-14 22:15:00Z")]
    );
}

#[test]
fn a_prompt_without_time_created_keeps_the_models_timestamp() {
    let fx = Fx::new(&[("u1", "user", r#"{"text":"Hello"}"#)]);
    let content = fx.content(0, SectionKey::Prompt);
    assert_eq!(labels(&content, "Prompt"), ["timestamp"]);
}

#[test]
fn a_prompt_has_the_environment_of_its_row() {
    let fx = Fx::new(&[(
        "u1",
        "user",
        r#"{"time":{"created":1700000000000},"text":"Hello","snapshot":{"start":"s1","end":"s2"}}"#,
    )]);
    let content = fx.content(0, SectionKey::Prompt);
    assert_eq!(titles(&content), ["Prompt", "Environment"]);
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [("snapshot.start", "s1"), ("snapshot.end", "s2")]
    );
}

#[test]
fn a_system_section_shows_its_timestamp_then_the_snapshot_of_its_first_row() {
    let note = |snap: &str| {
        format!(
            r#"{{"time":{{"created":1700000000500}},"text":"note","snapshot":{{"start":"{snap}"}}}}"#
        )
    };
    let (first, second) = (note("first"), note("second"));
    let fx = Fx::new(&[
        ("s1", "synthetic", &first),
        ("s2", "system", &second),
        ("u1", "user", USER),
    ]);
    let turns = fx.turns();
    let id = turns[0]
        .sources()
        .get(&SourceKey::Item(ItemId::from("injected:s1")))
        .unwrap();
    assert_eq!(id.rows().unwrap().section.as_ref().unwrap().row, "s1");
    let content = fx.item(0, "injected:s1");
    assert_eq!(titles(&content), ["Environment"]);
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [
            ("timestamp", "2023-11-14 22:13:20Z"),
            ("snapshot.start", "first")
        ]
    );
}

#[test]
fn an_injected_section_in_an_assistant_turn_references_its_first_row() {
    let note = r#"{"time":{"created":1700000000500},"text":"note","snapshot":{"start":"inj"}}"#;
    let fx = Fx::new(&[
        ("u1", "user", USER),
        ("a1", "assistant", FULL),
        ("s9", "synthetic", note),
        (
            "a2",
            "assistant",
            r#"{"content":[{"type":"text","text":"more"}]}"#,
        ),
    ]);
    let content = fx.item(1, "injected:s9");
    assert_eq!(
        rows(&content, "Environment").unwrap().last(),
        Some(&("snapshot.start", "inj"))
    );
    assert!(
        rows(&content, "Turn").is_some(),
        "an Assistant turn's section"
    );
}

#[test]
fn the_footer_has_the_turn_group_and_nothing_read_from_a_row() {
    let content = full().content(1, SectionKey::Footer);
    assert_eq!(titles(&content), ["Turn"]);
    let turn = rows(&content, "Turn").unwrap();
    assert!(turn.contains(&("provider", "acme")), "{turn:?}");
}

#[test]
fn a_model_change_section_references_its_row() {
    let first = r#"{"model":{"id":"one"},"content":[{"type":"text","text":"a"}]}"#;
    let second =
        r#"{"model":{"id":"two"},"snapshot":{"start":"s"},"content":[{"type":"text","text":"b"}]}"#;
    let fx = Fx::new(&[
        ("u1", "user", USER),
        ("a1", "assistant", first),
        ("a2", "assistant", second),
    ]);
    let content = fx.item(1, "model:a2");
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [("snapshot.start", "s")]
    );
}

// -- a row without the fields

#[test]
fn a_row_without_the_fields_leaves_them_out_and_is_not_unavailable() {
    let fx = Fx::new(&[
        ("u1", "user", r#"{"text":"hi"}"#),
        (
            "a1",
            "assistant",
            r#"{"content":[{"type":"text","text":"hi"}]}"#,
        ),
    ]);
    let content = fx.item(1, "a1:0");
    assert_eq!(rows(&content, "Message").unwrap(), [("row id", "a1")]);
    assert!(rows(&content, "Environment").is_none());
    for title in ["Turn", "Message"] {
        assert!(!has_unavailable(&content, title));
    }
    let groups = fx.read(1, SourceKey::Item(ItemId::from("a1:0")));
    assert!(groups.iter().all(|g| !g.unavailable));
}

#[test]
fn an_empty_object_is_a_row_without_the_fields() {
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", "{}")]);
    let groups = fx.read(1, SourceKey::Footer);
    assert!(groups.iter().all(|g| !g.unavailable), "{groups:?}");
}

#[test]
fn a_value_of_an_unexpected_json_type_is_left_out_without_unavailable() {
    // The assembler read the row as it was; OpenCode then wrote odd types.
    let fx = full();
    let turns = fx.turns();
    let odd = r#"{"time":{"created":"soon","streamed":1.5,"completed":[1]},"agent":7,"model":{"providerID":{"a":1}},"content":[{"type":"text","text":"hi"},{"type":"tool","id":"call-1","name":"shell","state":{"status":"completed","input":{}},"time":{"created":"x"}}],"finish":5,"rawFinish":["x"],"error":12,"snapshot":{"start":false,"end":null},"cost":0.5}"#;
    fx.set_data("a1", odd);
    let text = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:0")));
    assert_eq!(rows(&text, "Message").unwrap(), [("row id", "a1")]);
    assert!(rows(&text, "Environment").is_none());
    let turn = labels(&text, "Turn");
    assert!(
        !turn.contains(&"provider") && !turn.contains(&"agent"),
        "{turn:?}"
    );
    let tool = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("call-1")));
    assert_eq!(labels(&tool, "Tool call"), ["duration", "status"]);
    for content in [&text, &tool] {
        for title in titles(content) {
            assert!(!has_unavailable(content, title), "{title}");
        }
    }
    // Whole objects of the wrong type too.
    fx.set_data(
        "a1",
        r#"{"time":"now","model":[1],"snapshot":"x","content":[{"type":"text","text":"hi"}]}"#,
    );
    let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:0")));
    assert_eq!(rows(&content, "Message").unwrap(), [("row id", "a1")]);
    assert!(!has_unavailable(&content, "Message"));
    assert!(!has_unavailable(&content, "Turn"));
}

#[test]
fn a_time_that_is_out_of_range_is_left_out() {
    let data =
        r#"{"time":{"created":9223372036854775807},"content":[{"type":"text","text":"hi"}]}"#;
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", data)]);
    let content = fx.item(1, "a1:0");
    assert_eq!(rows(&content, "Message").unwrap(), [("row id", "a1")]);
}

// -- the error row

#[test]
fn the_error_row_says_what_the_notice_of_the_same_row_says() {
    let errors = [
        r#""error":{"type":"aborted"}"#,
        r#""error":{"type":"aborted","message":"user cancelled"}"#,
        r#""error":{"type":"APIError","message":"overloaded"}"#,
        r#""error":{"type":"APIError"}"#,
        r#""error":{"message":"only a message"}"#,
        r#""error":"a plain string""#,
        r#""error":{"type":"  spaced  ","message":" text\n"}"#,
        r#""error":{}"#,
    ];
    for error in errors {
        let data =
            format!(r#"{{"finish":"error",{error},"content":[{{"type":"text","text":"x"}}]}}"#);
        let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
        let turns = fx.turns();
        let Turn::Assistant(turn) = &*turns[1] else {
            panic!()
        };
        let notice = turn
            .items
            .iter()
            .find_map(|item| match item {
                Item::Notice { text, .. } => Some(text.clone()),
                _ => None,
            })
            .expect("a notice");
        let content = fx.item(1, "a1:0");
        let row = rows(&content, "Message")
            .unwrap()
            .into_iter()
            .find(|(label, _)| *label == "error")
            .map(|(_, value)| value.to_owned());
        assert_eq!(row.as_deref(), Some(notice.as_str()), "{error}");
    }
}

#[test]
fn an_aborted_error_is_the_warning_text_and_an_absent_error_has_no_row() {
    let aborted = Fx::new(&[
        ("u1", "user", USER),
        (
            "a1",
            "assistant",
            r#"{"finish":"error","error":{"type":"aborted","message":"stop"},"content":[{"type":"text","text":"x"}]}"#,
        ),
    ]);
    let turns = aborted.turns();
    let Turn::Assistant(turn) = &*turns[1] else {
        panic!()
    };
    assert!(turn.items.iter().any(
        |i| matches!(i, Item::Notice { level: NoticeLevel::Warning, text, .. } if text == "aborted: stop")
    ));
    let content = aborted.item(1, "a1:0");
    assert!(
        rows(&content, "Message")
            .unwrap()
            .contains(&("error", "aborted: stop"))
    );
    // A normal ending has no error row.
    let content = Fx::new(&[
        ("u1", "user", USER),
        (
            "a1",
            "assistant",
            r#"{"finish":"stop","content":[{"type":"text","text":"x"}]}"#,
        ),
    ])
    .item(1, "a1:0");
    assert_eq!(
        rows(&content, "Message").unwrap(),
        [("row id", "a1"), ("finish", "stop")]
    );
}

// -- an unreadable source

fn assert_unavailable(content: &DetailsContent, groups: &[&str]) {
    for title in groups {
        let rows = rows(content, title).unwrap_or_else(|| panic!("no {title} in {content:?}"));
        assert_eq!(
            rows.iter().filter(|(l, _)| *l == "unavailable").count(),
            1,
            "{title}: {rows:?}"
        );
        assert_eq!(rows.last(), Some(&("unavailable", "")), "{title}");
    }
    let unavailable = content
        .groups()
        .iter()
        .flat_map(|g| &g.rows)
        .filter(|r| r.label == "unavailable");
    assert!(unavailable.clone().all(|r| r.muted));
    assert_eq!(unavailable.count(), groups.len(), "{content:?}");
}

#[test]
fn a_database_that_is_gone_makes_each_group_unavailable() {
    let fx = full();
    std::fs::remove_file(&fx.db.path).unwrap();
    let turns = {
        // The turns were made while the database was there.
        let fx2 = full();
        fx2.turns()
    };
    let turn = &turns[1];
    let id = SectionId {
        turn: turn.id().clone(),
        key: SectionKey::Item(ItemId::from("a1:0")),
    };
    let source = section_source(turn, &id).unwrap();
    let read = section_details(&fx.db.path, &source);
    let model = model_details(turn, &id, &formatter()).unwrap();
    let content = compose(model, &read, &formatter());
    assert_unavailable(&content, &["Turn", "Message", "Environment"]);
    // What the model knows is still there.
    assert!(
        rows(&content, "Turn")
            .unwrap()
            .contains(&("model", "acme/coder-1"))
    );
}

#[test]
fn a_row_that_was_deleted_makes_its_groups_unavailable() {
    let fx = full();
    let turns = fx.turns();
    let turn = &turns[1];
    let id = |key| SectionId {
        turn: turn.id().clone(),
        key,
    };
    fx.delete("a1");
    for key in [
        SectionKey::Item(ItemId::from("a1:0")),
        SectionKey::Item(ItemId::from("notice:a1")),
    ] {
        let id = id(key);
        let source = section_source(turn, &id).unwrap();
        let read = section_details(&fx.db.path, &source);
        let content = compose(
            model_details(turn, &id, &formatter()).unwrap(),
            &read,
            &formatter(),
        );
        assert_unavailable(&content, &["Turn", "Message", "Environment"]);
    }
    // A tool call: its own group too.
    let id = id(SectionKey::Item(ItemId::from("call-1")));
    let source = section_source(turn, &id).unwrap();
    let content = compose(
        model_details(turn, &id, &formatter()).unwrap(),
        &section_details(&fx.db.path, &source),
        &formatter(),
    );
    assert_unavailable(&content, &["Turn", "Tool call", "Environment"]);
    assert_eq!(
        rows(&content, "Tool call").unwrap(),
        [("duration", "1s"), ("status", "ok"), ("unavailable", "")]
    );
}

#[test]
fn a_data_that_does_not_parse_makes_its_groups_unavailable() {
    for bad in [
        "{not json",
        "[]",
        "\"text\"",
        "12",
        "",
        "{\"a\":1} trailing",
    ] {
        let fx = full();
        let turns = fx.turns();
        fx.set_data("a1", bad);
        let turn = &turns[1];
        let id = SectionId {
            turn: turn.id().clone(),
            key: SectionKey::Item(ItemId::from("a1:0")),
        };
        let source = section_source(turn, &id).unwrap();
        let content = compose(
            model_details(turn, &id, &formatter()).unwrap(),
            &section_details(&fx.db.path, &source),
            &formatter(),
        );
        assert_unavailable(&content, &["Turn", "Message", "Environment"]);
    }
}

#[test]
fn a_tool_part_that_is_gone_makes_only_the_tool_group_unavailable() {
    let fx = full();
    let turns = fx.turns();
    fx.set_data(
        "a1",
        r#"{"content":[{"type":"text","text":"x"}],"snapshot":{"start":"s"}}"#,
    );
    let turn = &turns[1];
    let id = SectionId {
        turn: turn.id().clone(),
        key: SectionKey::Item(ItemId::from("call-1")),
    };
    let source = section_source(turn, &id).unwrap();
    let content = compose(
        model_details(turn, &id, &formatter()).unwrap(),
        &section_details(&fx.db.path, &source),
        &formatter(),
    );
    assert!(has_unavailable(&content, "Tool call"));
    assert!(!has_unavailable(&content, "Turn"));
    assert!(!has_unavailable(&content, "Environment"));
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [("snapshot.start", "s")]
    );
}

#[test]
fn the_prompt_row_deleted_keeps_the_models_timestamp_then_unavailable() {
    let fx = Fx::new(&[("u1", "user", USER)]);
    let turns = fx.turns();
    fx.delete("u1");
    let turn = &turns[0];
    let id = SectionId {
        turn: turn.id().clone(),
        key: SectionKey::Prompt,
    };
    let source = section_source(turn, &id).unwrap();
    let content = compose(
        model_details(turn, &id, &formatter()).unwrap(),
        &section_details(&fx.db.path, &source),
        &formatter(),
    );
    assert_eq!(labels(&content, "Prompt"), ["timestamp", "unavailable"]);
    assert_unavailable(&content, &["Prompt", "Environment"]);
}

#[test]
fn a_section_read_from_a_transcript_gives_nothing() {
    use crate::model::{ByteSource, LineRefs, SectionSource, SourceRef};
    let source = SectionSource {
        kind: SourceKind::Message,
        refs: SectionRefs::Lines(LineRefs {
            line: SourceRef {
                bytes: ByteSource::Memory(Arc::from(&b"{}"[..])),
                range: 0..2,
            },
            result: None,
        }),
    };
    assert!(section_details(&full().db.path, &source).is_empty());
}

// -- the database stays as it was

#[test]
fn reading_details_leaves_the_database_untouched() {
    let fx = full();
    let turns = fx.turns();
    let before = std::fs::read(&fx.db.path).unwrap();
    let files = fx.db.files();

    let turn = &turns[1];
    for key in [
        SectionKey::Item(ItemId::from("a1:0")),
        SectionKey::Item(ItemId::from("call-1")),
        SectionKey::Footer,
    ] {
        let id = SectionId {
            turn: turn.id().clone(),
            key,
        };
        let source = section_source(turn, &id).unwrap();
        assert!(!section_details(&fx.db.path, &source).is_empty());
    }
    let id = SectionId {
        turn: turns[0].id().clone(),
        key: SectionKey::Prompt,
    };
    let source = section_source(&turns[0], &id).unwrap();

    // The connection the reader works with is the read-only one: it takes a
    // `&Db` that was opened read-only and refuses to write.
    let db = Db::open(&fx.db.path).unwrap();
    assert_eq!(db.mode(), OpenMode::ReadOnly);
    assert!(!read_groups(Some(&db), &source).is_empty());
    let err = db
        .select("DELETE FROM session_message", [], |_| Ok(()))
        .unwrap_err();
    assert!(format!("{err:#}").contains("not a SELECT"), "{err:#}");
    drop(db);

    assert_eq!(std::fs::read(&fx.db.path).unwrap(), before);
    assert_eq!(fx.db.files(), files, "no -journal, -wal or -shm left");
}

#[test]
fn no_database_connection_means_every_group_is_unavailable() {
    let fx = full();
    let turns = fx.turns();
    let id = SectionId {
        turn: turns[1].id().clone(),
        key: SectionKey::Item(ItemId::from("a1:0")),
    };
    let source = section_source(&turns[1], &id).unwrap();
    let groups = read_groups(None, &source);
    let kinds: Vec<SourceGroupKind> = groups.iter().map(|g| g.kind).collect();
    assert_eq!(
        kinds,
        [
            SourceGroupKind::Turn,
            SourceGroupKind::Message,
            SourceGroupKind::Environment
        ]
    );
    assert!(groups.iter().all(|g| g.unavailable));
}

// -- the assembler's references

#[test]
fn a_row_replaced_in_place_keeps_the_references_of_a_fresh_assembly() {
    let rows = |text: &str| {
        let data = format!(
            r#"{{"model":{{"id":"m"}},"content":[{{"type":"text","text":"{text}"}},{{"type":"tool","id":"c1","name":"shell","state":{{"status":"pending","input":{{}}}}}}]}}"#
        );
        Row {
            id: "a1".into(),
            kind: "assistant".into(),
            seq: 2,
            created: 2000,
            updated: 2000,
            data,
        }
    };
    let user = Row {
        id: "u1".into(),
        kind: "user".into(),
        seq: 1,
        created: 1000,
        updated: 1000,
        data: USER.into(),
    };
    let mut asm = Assembler::new();
    asm.push_row(&user);
    asm.push_row(&rows("a"));
    asm.finish();
    for text in ["ab", "abc", "abcd"] {
        asm.replace_row(&rows(text));
    }
    let mut fresh = Assembler::new();
    fresh.push_row(&user);
    fresh.push_row(&rows("abcd"));
    fresh.finish();
    assert_eq!(asm.turns(), fresh.turns(), "references do not pile up");
}

#[test]
fn a_notice_that_goes_loses_its_reference() {
    let row = |finish: &str| Row {
        id: "a1".into(),
        kind: "assistant".into(),
        seq: 2,
        created: 2000,
        updated: 2000,
        data: format!(r#"{{"finish":"{finish}","content":[{{"type":"text","text":"x"}}]}}"#),
    };
    let mut asm = Assembler::new();
    asm.push_row(&row("length"));
    asm.finish();
    let key = SourceKey::Item(ItemId::from("notice:a1"));
    assert!(asm.turns()[0].sources().get(&key).unwrap().rows().is_some());
    asm.replace_row(&row("stop"));
    let Turn::Assistant(turn) = &*asm.turns()[0] else {
        panic!()
    };
    assert!(turn.items.iter().all(|i| i.id().as_str() != "notice:a1"));
    assert_eq!(turn.sources.get(&key), None, "nothing is kept for it");
    let mut fresh = Assembler::new();
    fresh.push_row(&row("stop"));
    fresh.finish();
    assert_eq!(asm.turns(), fresh.turns());
}

#[test]
fn a_value_read_is_time_text_or_nothing_else() {
    // Times are points in time (formatted by the UI), the rest is text.
    let fx = full();
    let groups = fx.read(1, SourceKey::Item(ItemId::from("a1:0")));
    let message = groups
        .iter()
        .find(|g| g.kind == SourceGroupKind::Message)
        .unwrap();
    let kind = |label: &str| {
        &message
            .rows
            .iter()
            .find(|r| r.label == label)
            .unwrap_or_else(|| panic!("{label}"))
            .value
    };
    assert!(matches!(kind("time.created"), SourceValue::Time(_)));
    assert!(matches!(kind("row id"), SourceValue::Text(_)));
    assert!(matches!(kind("error"), SourceValue::Text(_)));
}

#[test]
fn a_tool_part_sent_again_by_a_later_row_is_read_from_that_row() {
    let part = |created: i64, completed: i64| {
        format!(
            r#"{{"type":"tool","id":"c1","name":"shell","state":{{"status":"completed","input":{{}},"content":[]}},"time":{{"created":{created},"completed":{completed}}}}}"#
        )
    };
    let first = format!(
        r#"{{"snapshot":{{"start":"first"}},"content":[{}]}}"#,
        part(1_700_000_001_000, 1_700_000_002_000)
    );
    let later = format!(
        r#"{{"snapshot":{{"start":"later"}},"content":[{}]}}"#,
        part(1_700_000_500_000, 1_700_000_505_000)
    );
    let fx = Fx::new(&[
        ("u1", "user", USER),
        ("a1", "assistant", &first),
        ("a2", "assistant", &later),
    ]);
    let content = fx.item(1, "c1");
    assert_eq!(
        rows(&content, "Tool call").unwrap(),
        [
            ("start", "2023-11-14 22:21:40Z"),
            ("duration", "5s"),
            ("status", "ok")
        ]
    );
    assert_eq!(
        rows(&content, "Environment").unwrap(),
        [("snapshot.start", "later")]
    );
}

#[test]
fn parts_that_are_not_objects_do_not_hide_a_tool_part_or_move_it() {
    let fx = full();
    let turns = fx.turns();
    let tool = r#"{"type":"tool","id":"call-1","name":"shell","state":{"status":"completed","input":{}},"time":{"created":1700000001500,"completed":1700000002500}}"#;
    fx.set_data(
        "a1",
        &format!(r#"{{"content":[5,null,"s",[1],{tool},true]}}"#),
    );
    let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("call-1")));
    assert_eq!(
        rows(&content, "Tool call").unwrap(),
        [
            ("start", "2023-11-14 22:13:21Z"),
            ("duration", "1s"),
            ("status", "ok")
        ]
    );
    assert!(!has_unavailable(&content, "Turn"));
    // A part without an id is found by its original position.
    let no_id = tool.replace(r#""id":"call-1","#, "");
    let data = format!(r#"{{"content":[5,null,{no_id}]}}"#);
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
    let turns = fx.turns();
    fx.set_data("a1", &format!(r#"{{"content":["x",5,null,{no_id}]}}"#));
    // Position 3 now: the part is no longer where the assembler saw it.
    let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:2")));
    assert!(has_unavailable(&content, "Tool call"));
    fx.set_data("a1", &data);
    let content = fx.content_of(&turns, 1, SectionKey::Item(ItemId::from("a1:2")));
    assert_eq!(
        labels(&content, "Tool call"),
        ["start", "duration", "status"]
    );
    assert!(!has_unavailable(&content, "Tool call"));
}

#[test]
fn an_error_that_says_nothing_is_no_row_unless_the_notice_says_error() {
    for error in [
        r#"{}"#,
        r#"{"type":5,"message":true}"#,
        r#"{"type":"  ","message":""}"#,
        r#""""#,
        r#"7"#,
        r#"null"#,
    ] {
        let data = format!(
            r#"{{"finish":"stop","error":{error},"content":[{{"type":"text","text":"x"}}]}}"#
        );
        let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", &data)]);
        let content = fx.item(1, "a1:0");
        assert!(!labels(&content, "Message").contains(&"error"), "{error}");
        assert!(!has_unavailable(&content, "Message"), "{error}");
    }
    // An error that says something is a row whatever the finish.
    let data =
        r#"{"finish":"stop","error":{"message":"late"},"content":[{"type":"text","text":"x"}]}"#;
    let fx = Fx::new(&[("u1", "user", USER), ("a1", "assistant", data)]);
    assert!(
        rows(&fx.item(1, "a1:0"), "Message")
            .unwrap()
            .contains(&("error", "late"))
    );
}
