//! Tests of the rows the turns sidebar lists (product §3.2, §8.2): which turns
//! have an entry, the rows drawn from the entries, and how updates change
//! them.

use std::time::Duration;

use super::fixtures::*;
use super::*;
use crate::components::transcript::fixtures::with_id;
use crate::components::turn_block::fixtures as blocks;
use crate::model::Thinking;

fn with_stats(mut turn: Turn, secs: u64) -> Turn {
    if let Turn::Assistant(assistant) = &mut turn {
        assistant.stats = Some(stats(secs));
    }
    turn
}

#[test]
fn each_kind_is_listed_in_transcript_order_between_start_and_end() {
    let rows = rows_of(&wireframe());
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "Assistant",
            "|",
            "User",
            "|",
            "Assistant",
            "|",
            "Compaction",
            "|",
            "Assistant",
            "|",
            "End"
        ]
    );
    assert_eq!(rows.entry_count(), 6);
    assert_eq!(rows.row_count(), 15);
}

#[test]
fn start_and_end_are_not_selectable_and_have_no_entry() {
    let rows = rows_of(&wireframe());
    assert_eq!(rows.row(0), Some(Row::Start));
    assert_eq!(rows.row(rows.row_count() - 1), Some(Row::End));
    for row in 0..rows.row_count() {
        let selectable = rows.row(row).unwrap().selectable();
        assert_eq!(selectable, rows.entry_at_row(row).is_some(), "row {row}");
        let is_entry = matches!(rows.row(row), Some(Row::Entry(_)));
        assert_eq!(selectable, is_entry, "row {row}");
    }
    assert_eq!(rows.entry_at_row(0), None);
    assert_eq!(rows.entry_at_row(rows.row_count() - 1), None);
    assert_eq!(rows.entry_at_row(1), None, "a connector");
    assert_eq!(rows.entry_at_row(rows.row_count()), None);
    // The entry and its row map both ways.
    for entry in 0..rows.entry_count() {
        assert_eq!(
            rows.entry_at_row(TurnRows::row_of_entry(entry)),
            Some(entry)
        );
    }
}

#[test]
fn an_empty_list_is_start_a_connector_and_end() {
    let rows = rows_of(&[]);
    assert_eq!(kinds(&rows), ["Start", "|", "End"]);
    assert_eq!(rows.entry_count(), 0);
}

#[test]
fn connector_rows_join_the_rows_and_a_compaction_is_joined_like_any_other() {
    let rows = rows_of(&[user("u1"), compaction("c1"), assistant("a1", 5)]);
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "Compaction",
            "|",
            "Assistant",
            "|",
            "End"
        ]
    );
}

#[test]
fn two_adjacent_compactions_have_one_connector_between_them() {
    let rows = rows_of(&[user("u1"), compaction("c1"), compaction("c2"), user("u2")]);
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "Compaction",
            "|",
            "Compaction",
            "|",
            "User",
            "|",
            "End"
        ]
    );
}

#[test]
fn a_compaction_right_after_start_and_right_before_end() {
    let first = rows_of(&[compaction("c1"), user("u1")]);
    assert_eq!(
        kinds(&first),
        ["Start", "|", "Compaction", "|", "User", "|", "End"]
    );
    let last = rows_of(&[user("u1"), compaction("c1")]);
    assert_eq!(
        kinds(&last),
        ["Start", "|", "User", "|", "Compaction", "|", "End"]
    );
    let only = rows_of(&[compaction("c1")]);
    assert_eq!(kinds(&only), ["Start", "|", "Compaction", "|", "End"]);
}

#[test]
fn a_turn_with_only_redacted_thinking_or_only_system_content_has_no_entry() {
    assert_eq!(entry_for(&redacted_only("a1")), None);
    assert_eq!(entry_for(&system_only("u1")), None);
    let rows = rows_of(&[redacted_only("a1"), system_only("u1"), user("u2")]);
    assert_eq!(kinds(&rows), ["Start", "|", "User", "|", "End"]);
    // Their slots are kept: an update can give them an entry later.
    assert_eq!(rows.turn_count(), 3);
    assert_eq!(rows.entry_index(&id("a1")), None);
    assert_eq!(rows.entry_index(&id("u2")), Some(0));
}

#[test]
fn redacted_thinking_with_a_duration_has_an_entry_because_of_its_footer() {
    let turn = with_stats(redacted_only("a1"), 90);
    assert!(entry_for(&turn).is_some());
    let rows = rows_of(&[turn]);
    let (_, entry) = rows.entry(0).unwrap();
    assert_eq!(entry.duration, Some(Duration::from_secs(90)));
}

#[test]
fn a_turn_with_only_a_notice_has_an_entry() {
    let rows = rows_of(&[notice_only("a1")]);
    assert_eq!(kinds(&rows), ["Start", "|", "Assistant", "|", "End"]);
    assert_eq!(rows.entry(0).unwrap().1.duration, None);
}

#[test]
fn system_content_next_to_a_prompt_does_not_change_the_entry() {
    let with_prompt = with_id_user_items("u1");
    assert_eq!(
        entry_for(&with_prompt).map(|e| e.kind),
        Some(EntryKind::User)
    );
}

#[test]
fn an_assistant_turn_gaining_a_duration_shows_it_in_place() {
    let mut rows = rows_of(&[user("u1"), running("a1"), user("u2")]);
    assert_eq!(rows.entry(1).unwrap().1.duration, None);
    rows.apply(&updated(vec![assistant("a1", 4 * 60 + 3)]));
    assert_eq!(
        rows.entry(1).unwrap().1.duration,
        Some(Duration::from_secs(243))
    );
    assert_eq!(
        kinds(&rows),
        kinds(&rows_of(&[user("u1"), running("a1"), user("u2")]))
    );
}

#[test]
fn an_updated_turn_gaining_its_first_section_appears_in_its_transcript_position() {
    // `a1` has no section yet (redacted thinking, no stats).
    let mut rows = rows_of(&[
        user("u1"),
        redacted_only("a1"),
        user("u2"),
        compaction("c1"),
    ]);
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "User",
            "|",
            "Compaction",
            "|",
            "End"
        ]
    );
    rows.apply(&updated(vec![assistant("a1", 7)]));
    // Between the two User turns, where the turn is, not at the end.
    assert_eq!(
        kinds(&rows),
        [
            "Start",
            "|",
            "User",
            "|",
            "Assistant",
            "|",
            "User",
            "|",
            "Compaction",
            "|",
            "End"
        ]
    );
    assert_eq!(rows.entry_index(&id("a1")), Some(1));
    assert_eq!(rows.entry_index(&id("u2")), Some(2));
}

#[test]
fn an_updated_turn_losing_its_entry_leaves_the_list_and_keeps_its_place() {
    let mut rows = rows_of(&[user("u1"), assistant("a1", 7), user("u2")]);
    rows.apply(&updated(vec![redacted_only("a1")]));
    assert_eq!(
        kinds(&rows),
        ["Start", "|", "User", "|", "User", "|", "End"]
    );
    assert_eq!(rows.turn_count(), 3);
    assert_eq!(rows.entry_index(&id("a1")), None);
    assert_eq!(rows.entry_index(&id("u2")), Some(1));
    rows.apply(&updated(vec![assistant("a1", 9)]));
    assert_eq!(rows.entry_index(&id("a1")), Some(1), "back in its place");
    assert_eq!(rows.entry_index(&id("u2")), Some(2));
}

#[test]
fn an_appended_turn_with_a_known_id_updates_its_row_instead_of_adding_one() {
    let mut rows = rows_of(&[user("u1"), running("a1")]);
    rows.apply(&appended(vec![assistant("a1", 61)]));
    assert_eq!(rows.turn_count(), 2);
    assert_eq!(rows.entry_count(), 2);
    assert_eq!(
        rows.entry(1).unwrap().1.duration,
        Some(Duration::from_secs(61))
    );
    // A new turn is appended, with the same batch holding an update.
    rows.apply(&appended(vec![assistant("a1", 62), user("u2")]));
    assert_eq!(rows.turn_count(), 3);
    assert_eq!(
        rows.entry(1).unwrap().1.duration,
        Some(Duration::from_secs(62))
    );
    assert_eq!(rows.entry_index(&id("u2")), Some(2));
}

#[test]
fn an_updated_turn_with_an_unknown_id_is_ignored() {
    let mut rows = rows_of(&[user("u1")]);
    let before = rows.clone();
    rows.apply(&updated(vec![assistant("zz", 5)]));
    assert_eq!(rows, before);
}

#[test]
fn reset_clears_the_rows_and_other_updates_change_nothing() {
    let mut rows = rows_of(&wireframe());
    let before = rows.clone();
    for other in [
        ModelUpdate::Loaded,
        ModelUpdate::Progress(0.5),
        ModelUpdate::ParseWarnings(2),
    ] {
        rows.apply(&other);
        assert_eq!(rows, before);
    }
    rows.apply(&ModelUpdate::Reset);
    assert_eq!(kinds(&rows), ["Start", "|", "End"]);
    assert_eq!(rows.turn_count(), 0);
    assert_eq!(rows.entry_index(&id("u1")), None, "the map is cleared too");
}

#[test]
fn a_turn_update_rebuilds_only_the_turn_that_changed() {
    let turns = long_session(5_000);
    let mut rows = rows_of(&turns);
    let others: Vec<_> = (0..rows.entry_count())
        .map(|e| rows.entry(e).unwrap().1)
        .collect();
    let builds = entry_builds();
    rows.apply(&updated(vec![assistant("a4001", 12_345)]));
    assert_eq!(entry_builds() - builds, 1, "one entry derived for one turn");
    for (e, before) in others.iter().enumerate() {
        let (turn, now) = rows.entry(e).unwrap();
        if turn.as_str() == "a4001" {
            assert_eq!(now.duration, Some(Duration::from_secs(12_345)));
        } else {
            assert_eq!(now, *before, "entry {e}");
        }
    }
    let builds = entry_builds();
    rows.apply(&appended(vec![user("new")]));
    assert_eq!(entry_builds() - builds, 1);
}

#[test]
fn readable_thinking_is_a_section_and_redacted_thinking_is_not() {
    let readable = with_id(
        blocks::assistant(
            None,
            vec![blocks::thinking("th", Thinking::Text("hm".into()))],
            None,
        ),
        "a1",
    );
    assert!(entry_for(&readable).is_some());
    assert_eq!(entry_for(&redacted_only("a1")), None);
}
