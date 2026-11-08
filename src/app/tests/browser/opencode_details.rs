//! The details overlay with what OpenCode records (SPEC cycle 2 §5.2, §5.3),
//! through `App::update` on the OpenCode fixture database
//! (`tests/fixtures/opencode/fixture.sql`): the overlay opens, the adapter
//! reads the section's row and the overlay shows its fields; a row that is
//! gone leaves one muted `unavailable` row and the overlay open.

use super::details::overlay_rows;
use super::opencode::{OpenCodeRig, two_tabs};
use super::*;

/// The text section of `msg_main_04`: part 2 of its `content`.
const TEXT: &str = "msg_main_04:2";

/// Opens the fixture's `ses_main`, selects `item` and opens its details.
fn opened(width: u16, height: u16, item: &str) -> OpenCodeRig {
    let mut rig = two_tabs();
    rig.rig.app.update(Msg::Resized(width, height));
    rig.open("ses_main");
    rig.select_item(item);
    rig.rig.open_details();
    rig
}

/// The rows of the overlay, without the frame.
fn rows(rig: &mut OpenCodeRig, width: u16, height: u16) -> Vec<String> {
    overlay_rows(&mut rig.rig, width, height)
        .iter()
        .map(|row| {
            row.trim_matches(|c: char| c == '│' || c.is_whitespace())
                .to_owned()
        })
        .collect()
}

/// The value of the row `label` in the group headed `group`.
fn value(rows: &[String], group: &str, label: &str) -> String {
    let from = rows
        .iter()
        .position(|r| r == group)
        .unwrap_or_else(|| panic!("no {group} group in {rows:#?}"));
    rows[from + 2..]
        .iter()
        .take_while(|r| !r.is_empty())
        .find_map(|r| r.strip_prefix(label))
        .unwrap_or_else(|| panic!("no {label:?} in {group}: {rows:#?}"))
        .trim()
        .to_owned()
}

#[test]
fn a_text_section_shows_the_message_and_environment_rows_of_its_row() {
    let mut rig = opened(120, 40, TEXT);
    let rows = rows(&mut rig, 120, 40);
    // The Turn group: the model's rows, then cost, provider and agent.
    assert_eq!(value(&rows, "Turn", "model"), "acme/coder-1");
    assert_eq!(value(&rows, "Turn", "provider"), "acme");
    assert_eq!(value(&rows, "Turn", "agent"), "build");
    assert!(value(&rows, "Turn", "cost").starts_with('$'), "{rows:#?}");
    // Message, from the row.
    assert_eq!(value(&rows, "Message", "row id"), "msg_main_04");
    assert_eq!(
        value(&rows, "Message", "time.created"),
        "2026-10-28 09:00:03Z"
    );
    assert_eq!(value(&rows, "Message", "finish"), "stop");
    assert_eq!(value(&rows, "Message", "rawFinish"), "stop");
    // Environment, from the row.
    assert_eq!(
        value(&rows, "Environment", "snapshot.start"),
        "4a1c9e07d3b2"
    );
    assert_eq!(value(&rows, "Environment", "snapshot.end"), "7b2d5f3a91c4");
    assert!(
        !rows.iter().any(|r| r.starts_with("unavailable")),
        "{rows:#?}"
    );
    // The groups are in the order of the table.
    let at = |title: &str| rows.iter().position(|r| r == title).unwrap();
    assert!(at("Turn") < at("Message") && at("Message") < at("Environment"));
}

#[test]
fn a_tool_call_shows_its_start_duration_and_status_in_this_order() {
    let mut rig = opened(120, 40, "call-shell-1");
    let rows = rows(&mut rig, 120, 40);
    let tool = rows.iter().position(|r| r == "Tool call").unwrap();
    let labels: Vec<&str> = rows[tool + 2..]
        .iter()
        .take_while(|r| !r.is_empty())
        .filter_map(|r| r.split_whitespace().next())
        .collect();
    assert_eq!(labels, ["start", "duration", "status"], "{rows:#?}");
}

#[test]
fn a_row_that_is_gone_leaves_an_unavailable_row_and_the_overlay_open() {
    let mut rig = two_tabs();
    rig.open("ses_main");
    rig.select_item(TEXT);
    // The row is deleted after the session was loaded.
    rig.db
        .exec("DELETE FROM session_message WHERE id = 'msg_main_04';");
    rig.rig.open_details();
    let rows = rows(&mut rig, 120, 40);
    assert!(super::details::open(&rig.rig.app));
    for group in ["Turn", "Message", "Environment"] {
        let from = rows.iter().position(|r| r == group).unwrap();
        let group_rows: Vec<&String> = rows[from + 2..]
            .iter()
            .take_while(|r| !r.is_empty())
            .collect();
        assert_eq!(
            group_rows
                .iter()
                .filter(|r| r.starts_with("unavailable"))
                .count(),
            1,
            "{group}: {rows:#?}"
        );
    }
    // What the model knows is still shown.
    assert_eq!(value(&rows, "Turn", "model"), "acme/coder-1");
}

// -- screens

fn screen(width: u16, height: u16) -> String {
    let mut rig = opened(width, height, TEXT);
    screen_text(&mut rig.rig, width, height)
}

#[test]
fn the_details_of_an_opencode_text_section_80x24_text() {
    insta::assert_snapshot!(screen(80, 24));
}

#[test]
fn the_details_of_an_opencode_text_section_120x40_text() {
    insta::assert_snapshot!(screen(120, 40));
}
