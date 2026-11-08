//! The turns sidebar in the browser (product §5.1): where it sits, when it is
//! hidden, what it lists for the transcript on top, and how it follows the
//! updates, the theme and the mouse.

use ratatui::layout::Rect;
use tuirealm::event::{MouseButton, MouseEvent, MouseEventKind};

use super::subagent::subagent_rig;
use super::theme_selector::mounted_themes;
use super::*;
use crate::components::transcript::fixtures::{arc, reply};
use crate::components::{
    EntryKind, PRESENTATION_ATTR, SELECTED_ATTR, THEME_ATTR, TURN_ROWS_ATTR, TurnRows,
    VISIBLE_ROWS_ATTR, selected_from_value, sidebar_list_rows, theme_from_value,
};

fn transcript_viewport(app: &App) -> Size {
    let value = app
        .application()
        .query(&Id::Transcript, VIEWPORT_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

/// The rows the sidebar holds.
pub(super) fn sidebar_rows(app: &App) -> TurnRows {
    let value = app
        .application()
        .query(&Id::TurnSidebar, TURN_ROWS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value(&value).unwrap()
}

/// The kinds of the entries the sidebar lists, in order.
pub(super) fn entry_kinds(app: &App) -> Vec<EntryKind> {
    let rows = sidebar_rows(app);
    (0..rows.entry_count())
        .map(|i| rows.entry(i).unwrap().1.kind)
        .collect()
}

/// The first 30 columns (the pane) of each screen row from row 5 to the row
/// above the help line's blank row.
pub(super) fn pane_rows(rig: &mut Rig, width: u16, height: u16) -> Vec<String> {
    screen_text(rig, width, height)
        .lines()
        .skip(5)
        .take(usize::from(height) - 7)
        .map(|row| row.chars().take(30).collect())
        .collect()
}

fn resize(rig: &mut Rig, width: u16, height: u16) {
    rig.app.update(Msg::Resized(width, height));
}

// -- where it is

#[test]
fn at_120_columns_the_sidebar_is_mounted_and_laid_out_at_the_left() {
    let mut rig = rig();
    rig.open_loaded();
    assert!(rig.app.application().mounted(&Id::TurnSidebar));
    assert_eq!(
        rig.app.routing().area(Id::TurnSidebar),
        Some(Rect::new(0, 5, 30, 33))
    );
    assert_eq!(
        rig.app.routing().area(Id::Transcript),
        Some(Rect::new(31, 5, 89, 33))
    );
    assert_eq!(transcript_viewport(&rig.app), Size::new(89, 33));
    // Header and help line stay full width.
    assert_eq!(
        rig.app.routing().area(Id::BrowserHeader).unwrap().width,
        120
    );
    assert_eq!(
        rig.app.routing().area(Id::BrowserHelpLine).unwrap().width,
        120
    );
}

#[test]
fn below_90_columns_it_is_not_in_the_layout_and_the_transcript_has_the_full_width() {
    let mut rig = rig_with(theme("default"), 89, 30);
    rig.open_loaded();
    assert_eq!(rig.app.routing().area(Id::TurnSidebar), None);
    assert_eq!(
        rig.app.routing().area(Id::Transcript),
        Some(Rect::new(0, 5, 89, 23))
    );
    assert_eq!(transcript_viewport(&rig.app), Size::new(89, 23));
    let text = screen_text(&mut rig, 89, 30);
    assert!(!text.contains("Turns"), "{text}");
    assert!(!text.contains("┬   Start"), "{text}");
    let row: String = text.lines().nth(5).unwrap().chars().collect();
    assert!(row.starts_with("╭─ User "), "the transcript from column 0");
}

#[test]
fn at_90_columns_the_transcripts_viewport_is_59_wide() {
    let mut rig = rig_with(theme("default"), 90, 24);
    rig.open_loaded();
    assert_eq!(
        rig.app.routing().area(Id::TurnSidebar),
        Some(Rect::new(0, 5, 30, 17))
    );
    assert_eq!(transcript_viewport(&rig.app), Size::new(59, 17));
    let text = screen_text(&mut rig, 90, 24);
    assert!(text.lines().nth(5).unwrap().starts_with("╭─ [1] Turns"));
}

#[test]
fn resizing_across_90_columns_shows_and_hides_it_and_resizes_the_viewport() {
    let mut rig = rig_with(theme("default"), 80, 24);
    rig.open_loaded();
    assert_eq!(transcript_viewport(&rig.app), Size::new(80, 17));
    resize(&mut rig, 90, 24);
    assert!(rig.app.routing().area(Id::TurnSidebar).is_some());
    assert_eq!(transcript_viewport(&rig.app), Size::new(59, 17));
    resize(&mut rig, 89, 24);
    assert!(rig.app.routing().area(Id::TurnSidebar).is_none());
    assert_eq!(transcript_viewport(&rig.app), Size::new(89, 17));
    resize(&mut rig, 120, 40);
    assert_eq!(transcript_viewport(&rig.app), Size::new(89, 33));
}

#[test]
fn the_sidebar_is_told_how_many_list_rows_its_pane_shows() {
    let mut rig = rig();
    rig.open_loaded();
    let rows = |rig: &Rig| match rig
        .app
        .application()
        .query(&Id::TurnSidebar, VISIBLE_ROWS_ATTR)
        .unwrap()
        .unwrap()
        .into_attr()
    {
        AttrValue::Length(n) => n,
        other => panic!("{other:?}"),
    };
    // A 33-row pane: the border, a blank row, the list, the border.
    assert_eq!(rows(&rig), 30);
    assert_eq!(rows(&rig), sidebar_list_rows(33));
    resize(&mut rig, 100, 24);
    assert_eq!(rows(&rig), sidebar_list_rows(17));
}

#[test]
fn it_is_not_drawn_when_the_empty_session_replaces_the_transcript() {
    let mut rig = rig();
    rig.harness.script(BRIO_SESSION, vec![ModelUpdate::Loaded]);
    rig.open(BRIO_SESSION);
    assert!(rig.app.session_is_empty());
    assert_eq!(rig.app.routing().area(Id::TurnSidebar), None);
    assert_eq!(
        rig.app.routing().area(Id::EmptySession),
        Some(Rect::new(0, 5, 120, 33)),
        "the message keeps the full width"
    );
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.contains("Session file is empty"));
    assert!(!text.contains("Turns"), "{text}");
    assert!(!text.contains("┬   Start"), "{text}");
}

#[test]
fn the_sidebar_comes_when_the_first_turns_do_and_the_viewport_was_already_narrow() {
    let mut rig = rig();
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    rig.app.update(session_msg(&rig.app, ModelUpdate::Loaded));
    assert!(rig.app.session_is_empty());
    assert_eq!(transcript_viewport(&rig.app), Size::new(89, 33));
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(turns()[..2].to_vec()),
    ));
    assert!(!rig.app.session_is_empty());
    assert!(rig.app.routing().area(Id::TurnSidebar).is_some());
    assert_eq!(
        transcript_viewport(&rig.app),
        Size::new(89, 33),
        "no resize was needed"
    );
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant]
    );
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.contains("[1] Turns"), "{text}");
}

#[test]
fn scrolling_the_transcript_does_not_move_the_pane() {
    let mut rig = rig();
    rig.open_loaded();
    // The marker follows the selection: compare the pane without its column.
    let unmarked = |rows: Vec<String>| -> Vec<String> {
        rows.into_iter()
            .map(|row| {
                row.chars()
                    .enumerate()
                    .map(|(at, c)| if at == 3 { ' ' } else { c })
                    .collect()
            })
            .collect()
    };
    let before = unmarked(pane_rows(&mut rig, 120, 40));
    assert!(before.iter().any(|row| row.contains("Start")));
    for key in [
        Key::Char('J'),
        Key::Char('G'),
        Key::Char('g'),
        Key::Char('j'),
    ] {
        rig.press(key);
        rig.settle();
        assert_eq!(
            unmarked(pane_rows(&mut rig, 120, 40)),
            before,
            "after {key:?}"
        );
    }
    let wheel = |rig: &mut Rig, kind| {
        let area = rig.app.routing().area(Id::Transcript).unwrap();
        rig.app.update(Msg::Mouse(MouseEvent {
            kind,
            modifiers: KeyModifiers::NONE,
            column: area.x + 10,
            row: area.y + 5,
        }));
    };
    for _ in 0..4 {
        wheel(&mut rig, MouseEventKind::ScrollDown);
        assert_eq!(unmarked(pane_rows(&mut rig, 120, 40)), before);
    }
    assert_eq!(
        rig.app.routing().area(Id::TurnSidebar),
        Some(Rect::new(0, 5, 30, 33))
    );
}

// -- what it lists

#[test]
fn it_lists_the_turns_of_the_loaded_session() {
    let mut rig = rig();
    rig.open_loaded();
    // The fixture: User, Assistant, User, Assistant, Compaction, User,
    // Assistant, Assistant.
    assert_eq!(
        entry_kinds(&rig.app),
        [
            EntryKind::User,
            EntryKind::Assistant,
            EntryKind::User,
            EntryKind::Assistant,
            EntryKind::Compaction,
            EntryKind::User,
            EntryKind::Assistant,
            EntryKind::Assistant
        ]
    );
    let pane = pane_rows(&mut rig, 120, 40);
    assert_eq!(pane[0], "╭─ [1] Turns  ───────────────╮");
    assert_eq!(pane[2], "│  ┬   Start                 │");
    assert_eq!(pane[6], "│  ├   Assistant      5m 2s  │");
    assert!(pane.iter().any(|r| r.contains("├╶╶ Compaction ╶")));
    assert!(pane.iter().any(|r| r.contains("┴   End")));
    assert_eq!(pane[32], "╰────────────────────────────╯");
}

#[test]
fn the_selection_is_the_transcripts_and_the_sidebar_has_no_focus_in_the_running_app() {
    let mut rig = rig();
    rig.open_loaded();
    let selected = rig
        .app
        .application()
        .query(&Id::TurnSidebar, SELECTED_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    // The first turn: the transcript's selection starts on its first section.
    assert_eq!(selected_from_value(&selected), Some(Some(0)));
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn appended_turns_add_rows_and_updated_stats_show_the_duration_in_place() {
    let mut rig = rig();
    rig.open_loaded();
    let count = entry_kinds(&rig.app).len();
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(reply("t9", "A late reply."))]),
    ));
    assert_eq!(entry_kinds(&rig.app).len(), count + 1);
    let before = pane_rows(&mut rig, 120, 40);
    assert!(
        !before.iter().any(|r| r.contains("Assistant      1h 1m")),
        "{before:?}"
    );
    // The reply gets its stats.
    let mut done = reply("t9", "A late reply.");
    if let Turn::Assistant(assistant) = &mut done {
        assistant.stats = Some(crate::model::TurnStats {
            duration: Duration::from_secs(3_660),
            ..crate::components::turn_block::fixtures::stats()
        });
    }
    rig.app
        .update(session_msg(&rig.app, ModelUpdate::Updated(vec![arc(done)])));
    assert_eq!(entry_kinds(&rig.app).len(), count + 1, "no new row");
    let after = pane_rows(&mut rig, 120, 40);
    assert!(
        after.iter().any(|r| r.contains("Assistant      1h 1m  │")),
        "{after:?}"
    );
}

#[test]
fn an_appended_turn_with_a_known_id_does_not_add_a_row() {
    let mut rig = rig();
    rig.open_loaded();
    let count = entry_kinds(&rig.app).len();
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![turns()[1].clone()]),
    ));
    assert_eq!(entry_kinds(&rig.app).len(), count);
    assert_eq!(rig.turn_count(), turns().len());
}

#[test]
fn reset_clears_the_rows() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.update(session_msg(&rig.app, ModelUpdate::Reset));
    assert!(entry_kinds(&rig.app).is_empty());
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(turns()[..1].to_vec()),
    ));
    assert_eq!(entry_kinds(&rig.app), [EntryKind::User]);
}

#[test]
fn the_rows_of_the_transcript_and_the_sidebar_agree_after_live_updates() {
    let mut rig = rig();
    rig.open_loaded();
    // A new turn, a known id appended again, and an update gaining a
    // duration, as a live session sends them.
    let mut done = reply("t9", "A late reply.");
    if let Turn::Assistant(assistant) = &mut done {
        assistant.stats = Some(crate::components::turn_block::fixtures::stats());
    }
    for update in [
        ModelUpdate::Appended(vec![arc(reply("t9", "A late reply."))]),
        ModelUpdate::Appended(vec![turns()[1].clone(), arc(reply("t10", "Another."))]),
        ModelUpdate::Updated(vec![arc(done)]),
    ] {
        rig.app.update(session_msg(&rig.app, update));
    }
    let from_transcript: TurnRows = payload_from_value(
        &rig.app
            .application()
            .query(&Id::Transcript, TURN_ROWS_ATTR)
            .unwrap()
            .unwrap()
            .into_attr(),
    )
    .unwrap();
    assert_eq!(from_transcript.turn_count(), turns().len() + 2);
    assert_eq!(sidebar_rows(&rig.app), from_transcript);
}

#[test]
fn opening_another_session_starts_with_an_empty_list() {
    let mut rig = rig();
    rig.open_loaded();
    assert!(!entry_kinds(&rig.app).is_empty());
    rig.harness.script(NEWER_SESSION, Vec::new());
    rig.app.update(Msg::OpenSession(CLAUDE, id(NEWER_SESSION)));
    assert!(entry_kinds(&rig.app).is_empty());
}

// -- subagents

#[test]
fn an_open_subagent_shows_its_turns_and_going_back_shows_the_parents_including_late_ones() {
    let mut rig = subagent_rig(120, 40);
    let parent = entry_kinds(&rig.app);
    assert!(parent.len() >= 9, "{parent:?}");
    rig.open_subagent(1);
    // The story writer: its prompt and an answer.
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant]
    );
    let pane = pane_rows(&mut rig, 120, 40);
    assert_eq!(pane[2], "│  ┬   Start                 │");
    assert!(pane[8].contains("┴   End"), "{pane:?}");
    // The parent receives turns while it is covered: the sidebar still shows
    // the subagent's.
    rig.harness.send(
        0,
        ModelUpdate::Appended(vec![arc(reply("late", "A late reply of the parent."))]),
    );
    rig.settle();
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant]
    );
    rig.press(Key::Backspace);
    rig.pump_until(|app| app.subagent_depth() == 0);
    rig.settle();
    let back = entry_kinds(&rig.app);
    assert_eq!(back.len(), parent.len() + 1, "the late turn is listed");
    assert_eq!(back[..parent.len()], parent[..]);
    assert_eq!(back.last(), Some(&EntryKind::Assistant));
}

#[test]
fn a_nested_subagent_lists_its_own_turns_and_going_back_each_level_restores_the_one_below() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    // Select the Explore call in the story writer's transcript.
    rig.press(Key::Char('G'));
    rig.press(Key::Char('l'));
    rig.settle();
    rig.open_subagent(2);
    assert_eq!(rig.app.subagent_depth(), 2);
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant]
    );
    rig.press(Key::Backspace);
    rig.pump_until(|app| app.subagent_depth() == 1);
    rig.settle();
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant]
    );
    rig.press(Key::Backspace);
    rig.pump_until(|app| app.subagent_depth() == 0);
    assert!(entry_kinds(&rig.app).len() >= 9);
}

#[test]
fn updates_of_the_transcript_on_top_reach_the_sidebar_during_a_subagent() {
    let mut rig = subagent_rig(120, 40);
    rig.open_subagent(1);
    rig.harness.send(
        1,
        ModelUpdate::Appended(vec![arc(reply("sa2", "Another answer."))]),
    );
    rig.settle();
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant, EntryKind::Assistant]
    );
}

// -- the theme

#[test]
fn a_theme_preview_repaints_the_sidebar() {
    let mut rig = rig();
    rig.open_loaded();
    let original = rig.app.theme().clone();
    rig.open_themes();
    rig.move_to(Key::Down, "terminal");
    let theme = rig.app.theme().clone();
    assert_ne!(theme, original);
    let value = rig
        .app
        .application()
        .query(&Id::TurnSidebar, THEME_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    assert_eq!(theme_from_value(&value), Some(theme.clone()));
    assert!(
        mounted_themes(&rig.app)
            .iter()
            .any(|(id, _)| *id == Id::TurnSidebar)
    );
    rig.close_themes();
    // The preview was kept or cancelled; either way the colours of the
    // sidebar follow the active theme.
    let after = screen_buffer(&mut rig, 120, 40);
    let active = rig.app.theme().role(crate::theme::Role::MutedForeground);
    assert_eq!(after[(3, 8)].fg, active);
}

#[test]
fn previewing_a_theme_changes_the_cells_of_the_sidebar() {
    let mut rig = rig();
    rig.open_loaded();
    let before = screen_buffer(&mut rig, 120, 40);
    rig.open_themes();
    rig.move_to(Key::Down, "terminal");
    let during = screen_buffer(&mut rig, 120, 40);
    // The pane's `┬` on its first list row: muted foreground in each theme.
    assert_ne!(before[(3, 7)].fg, during[(3, 7)].fg);
    assert_eq!(
        during[(3, 7)].fg,
        rig.app.theme().role(crate::theme::Role::MutedForeground)
    );
}

// -- the mouse

fn click(rig: &mut Rig, column: u16, row: u16) {
    rig.app.update(Msg::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        modifiers: KeyModifiers::NONE,
        column,
        row,
    }));
}

fn selected(rig: &Rig) -> Option<crate::components::turn_block::SectionId> {
    let value = rig
        .app
        .application()
        .query(&Id::Transcript, PRESENTATION_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    payload_from_value::<crate::components::Presentation>(&value)
        .unwrap()
        .selected
}

/// The screen row that shows the folded thinking summary.
fn thinking_row(rig: &mut Rig) -> u16 {
    screen_text(rig, 120, 40)
        .lines()
        .position(|row| row.contains("▶ Thinking:"))
        .map(|row| u16::try_from(row).unwrap())
        .expect("a thinking section is on screen")
}

#[test]
fn a_click_on_the_transcript_at_120_columns_hits_the_same_section_shifted_by_the_sidebar() {
    // The same click, 31 columns further right than without the sidebar,
    // selects the section the narrow layout shows at that point.
    let mut rig = rig();
    rig.open_loaded();
    let row = thinking_row(&mut rig);
    let before = selected(&rig);
    click(&mut rig, 31 + 10, row);
    let after = selected(&rig).unwrap();
    assert_ne!(Some(after.clone()), before);
    assert!(
        matches!(after.key, crate::components::SectionKey::Item(_)),
        "{after:?}"
    );
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.contains("▼ Thinking"), "unfolded by the click");
    // The same click on the 89-column layout without the sidebar hits the
    // same section.
    let mut narrow = rig_with(theme("default"), 89, 40);
    narrow.open_loaded();
    let row = screen_text(&mut narrow, 89, 40)
        .lines()
        .position(|row| row.contains("▶ Thinking:"))
        .map(|row| u16::try_from(row).unwrap())
        .unwrap();
    click(&mut narrow, 10, row);
    assert_eq!(selected(&narrow), Some(after));
}

#[test]
fn clicks_on_the_sidebars_border_and_in_the_gap_do_nothing() {
    // (What a click on a row does is `sidebar_mouse`'s.)
    let mut rig = rig();
    rig.open_loaded();
    let row = thinking_row(&mut rig);
    let before = selected(&rig);
    for column in [0, 29, 30] {
        click(&mut rig, column, row);
        assert_eq!(selected(&rig), before, "column {column}");
    }
    assert!(rig.app.take_queued().is_empty());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!screen_text(&mut rig, 120, 40).contains("▼ Thinking"));
}

#[test]
fn the_wheel_in_the_gap_does_not_scroll_the_transcript() {
    // (The wheel over the pane selects entries: `sidebar_mouse`.)
    let mut rig = rig();
    rig.open_loaded();
    let before = screen_text(&mut rig, 120, 40);
    rig.app.update(Msg::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        modifiers: KeyModifiers::NONE,
        column: 30,
        row: 12,
    }));
    assert_eq!(screen_text(&mut rig, 120, 40), before);
}

// -- focus

#[test]
fn a_sidebar_that_has_the_focus_when_it_is_hidden_gives_it_to_the_transcript() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.application_mut().active(&Id::TurnSidebar).unwrap();
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
    resize(&mut rig, 100, 30);
    assert_eq!(rig.focus(), Some(Id::TurnSidebar), "still shown at 100");
    resize(&mut rig, 89, 30);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_sidebar_stays_mounted_under_the_selector_overlay() {
    let mut rig = rig();
    rig.open_loaded();
    assert!(rig.app.application().mounted(&Id::TurnSidebar));
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    assert!(rig.app.application().mounted(&Id::TurnSidebar), "under it");
}

/// The cycle 3 fixture `local_commands.jsonl`: the caveat, a `/effort` echo
/// and its output, a `/factory` prompt and a reply. The local command starts
/// no turn: the sidebar lists one User and one Assistant row. The turns go
/// through the fake harness, so the header is the rig's (project BRIO).
#[test]
fn browser_on_the_local_commands_fixture_120x40_text() {
    let bytes = include_bytes!("../../../tests/fixtures/claude/local_commands.jsonl");
    let session = crate::harness::claude::Assembler::from_memory(Arc::from(&bytes[..]));
    let mut rig = rig();
    rig.harness.script(
        BRIO_SESSION,
        vec![
            ModelUpdate::Appended(session.into_turns()),
            ModelUpdate::Progress(1.0),
            ModelUpdate::ParseWarnings(0),
            ModelUpdate::Loaded,
        ],
    );
    rig.open(BRIO_SESSION);
    assert_eq!(rig.turn_count(), 2);
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant]
    );
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}
