//! The turns sidebar follows the transcript (product §3.2): its selected row is
//! the turn that contains the transcript's selected section, whichever path
//! moved that selection, and it stays visible.

use ratatui::layout::Rect;
use tuirealm::command::Cmd;
use tuirealm::event::{MouseEvent, MouseEventKind};

use super::sidebar::{entry_kinds, pane_rows, sidebar_rows};
use super::subagent::{subagent_rig, subagent_rig_after};
use super::*;
use crate::components::transcript::fixtures::{arc, prompt, reply};
use crate::components::turn_block::SectionId;
use crate::components::{
    EntryKind, SELECTED_ATTR, SELECTED_SECTION_ATTR, SELECTED_TURN_ATTR, payload_value,
    selected_from_value,
};

// -- what the tests read

/// The entry the sidebar has selected.
pub(super) fn sidebar_entry(app: &App) -> Option<usize> {
    let value = app
        .application()
        .query(&Id::TurnSidebar, SELECTED_ATTR)
        .unwrap()
        .unwrap()
        .into_attr();
    selected_from_value(&value).expect("a selection value")
}

/// The selected section of the transcript on top.
pub(super) fn section(app: &App) -> Option<SectionId> {
    let value = app
        .application()
        .query(&app.top_transcript(), SELECTED_SECTION_ATTR)
        .unwrap()?
        .into_attr();
    payload_from_value(&value)
}

/// The entry of the turn that contains the transcript's selected section.
fn followed_entry(app: &App) -> Option<usize> {
    sidebar_rows(app).entry_index(&section(app)?.turn)
}

/// The sidebar is on the turn of the transcript's selection. Returns the
/// entry.
fn assert_follows(app: &App, what: &str) -> usize {
    let expected = followed_entry(app).unwrap_or_else(|| panic!("{what}: no entry to follow"));
    assert_eq!(sidebar_entry(app), Some(expected), "{what}");
    expected
}

/// The row of the pane (counted from the first list row) that shows the `▒`.
fn marker_row(rig: &mut Rig, width: u16, height: u16) -> Option<usize> {
    pane_rows(rig, width, height)
        .iter()
        .skip(2)
        .position(|row| row.chars().nth(3) == Some('▒'))
}

/// Turns `u0`, `a0`, `u1`, `a1`, …: `pairs` of a prompt and a reply, each a
/// single section, so that `j` and `]` do the same.
pub(super) fn long_turns(pairs: usize) -> Vec<Arc<Turn>> {
    (0..pairs)
        .flat_map(|i| {
            [
                arc(prompt(&format!("u{i}"), &format!("Question {i}"))),
                arc(reply(&format!("a{i}"), &format!("Answer {i}"))),
            ]
        })
        .collect()
}

impl Rig {
    /// Opens the BRIO session with `turns`, loaded.
    pub(super) fn open_turns(&mut self, turns: Vec<Arc<Turn>>) {
        self.harness.script(
            BRIO_SESSION,
            vec![
                ModelUpdate::Appended(turns),
                ModelUpdate::Progress(1.0),
                ModelUpdate::ParseWarnings(0),
                ModelUpdate::Loaded,
            ],
        );
        self.open(BRIO_SESSION);
    }

    pub(super) fn press_ctrl(&self, c: char) {
        self.key(Key::Char(c), KeyModifiers::CONTROL);
    }

    pub(super) fn wheel(&mut self, kind: MouseEventKind) {
        let area = self.app.routing().area(Id::Transcript).unwrap();
        self.app.update_routed(Msg::Pointer(MouseEvent {
            kind,
            modifiers: KeyModifiers::NONE,
            column: area.x + 10,
            row: area.y + 3,
        }));
    }
}

pub(super) fn long_rig(pairs: usize, width: u16, height: u16) -> Rig {
    let mut rig = rig_with(theme("default"), width, height);
    rig.open_turns(long_turns(pairs));
    rig
}

// -- the keys

#[test]
fn the_selection_starts_on_the_turn_of_the_transcripts_first_section() {
    let mut rig = rig();
    rig.open_loaded();
    assert_eq!(assert_follows(&rig.app, "after the load"), 0);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn j_and_k_move_the_sidebar_selection_with_the_transcripts() {
    let mut rig = rig();
    rig.open_loaded();
    let mut seen = vec![assert_follows(&rig.app, "start")];
    for n in 0..14 {
        rig.press(Key::Char('j'));
        rig.settle();
        seen.push(assert_follows(&rig.app, &format!("after j #{n}")));
    }
    assert!(seen.windows(2).all(|w| w[0] <= w[1]), "{seen:?}");
    assert!(seen.last() >= Some(&4), "it moved on: {seen:?}");
    let top = *seen.last().unwrap();
    for n in 0..14 {
        rig.press(Key::Char('k'));
        rig.settle();
        assert_follows(&rig.app, &format!("after k #{n}"));
    }
    assert!(sidebar_entry(&rig.app) < Some(top));
    assert_eq!(sidebar_entry(&rig.app), Some(0), "back at the first turn");
}

#[test]
fn the_turn_keys_move_the_sidebar_one_entry_at_a_time() {
    let mut rig = rig();
    rig.open_loaded();
    for expected in 1..=5 {
        rig.press(Key::Char(']'));
        rig.settle();
        assert_eq!(assert_follows(&rig.app, "after ]"), expected);
    }
    // Entry 4 is the Compaction: the next turn block is a User block.
    rig.press(Key::Char('['));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "after ["), 4);
    rig.press(Key::Char('['));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "after ["), 3);
}

#[test]
fn the_page_keys_and_the_ends_move_the_sidebar_selection_with_the_transcript() {
    let mut rig = rig_with(theme("default"), 120, 20);
    rig.open_turns(long_turns(10));
    assert_eq!(assert_follows(&rig.app, "start"), 0);
    let mut last = 0;
    for (key, modifiers, name) in [
        (Key::PageDown, KeyModifiers::NONE, "PgDn"),
        (Key::Char('d'), KeyModifiers::CONTROL, "Ctrl-d"),
        (Key::PageDown, KeyModifiers::NONE, "PgDn"),
    ] {
        rig.key(key, modifiers);
        rig.settle();
        let entry = assert_follows(&rig.app, name);
        assert!(entry > last, "{name}: {entry} after {last}");
        last = entry;
    }
    rig.key(Key::PageUp, KeyModifiers::NONE);
    rig.settle();
    assert!(assert_follows(&rig.app, "PgUp") < last);
    rig.press_ctrl('u');
    rig.settle();
    assert_follows(&rig.app, "Ctrl-u");
    rig.press(Key::Char('G'));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "G"), 19);
    rig.press(Key::Char('g'));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "g"), 0);
    rig.press(Key::End);
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "End"), 19);
    rig.press(Key::Home);
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "Home"), 0);
}

#[test]
fn a_search_moves_the_sidebar_selection_to_the_turn_of_the_match() {
    let mut rig = build_rig(theme("default"), 120, 40, true);
    rig.open_turns(crate::components::transcript::fixtures::search_session());
    assert_eq!(sidebar_entry(&rig.app), Some(0));
    rig.search("cable");
    // `a2`, the fourth turn.
    assert_eq!(assert_follows(&rig.app, "after Enter"), 3);
    rig.press(Key::Esc);
    rig.settle();
    rig.search("usb");
    // The matches: u1? no, `a1` (twice), `u2`, `a2` and the tool output.
    let mut seen = vec![assert_follows(&rig.app, "first match")];
    for _ in 0..4 {
        rig.press(Key::Char('n'));
        rig.settle();
        seen.push(assert_follows(&rig.app, "after n"));
    }
    assert!(seen.iter().collect::<std::collections::BTreeSet<_>>().len() > 1);
    rig.press(Key::Char('N'));
    rig.settle();
    assert_follows(&rig.app, "after N");
    rig.press(Key::Esc);
    rig.settle();
    rig.search("camera");
    assert_eq!(assert_follows(&rig.app, "after the next search"), 0);
}

#[test]
fn the_sidebar_follows_while_the_search_prompt_has_the_focus() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char('G'));
    rig.settle();
    rig.open_prompt();
    assert_eq!(rig.focus(), Some(Id::Prompt));
    // The following transcript moves its selection to the new last turn.
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(reply("late", "Late."))]),
    ));
    assert_eq!(rig.focus(), Some(Id::Prompt), "the prompt keeps the focus");
    assert_eq!(assert_follows(&rig.app, "under the prompt"), 8);
}

// -- subagents

#[test]
fn opening_a_subagent_shows_the_row_of_its_selection_and_going_back_restores_the_parents() {
    let mut rig = subagent_rig(120, 40);
    let parent_entry = assert_follows(&rig.app, "the parent");
    let parent_section = section(&rig.app);
    let parent_rows = sidebar_rows(&rig.app);
    assert!(parent_entry >= 8, "the last turn: {parent_entry}");

    rig.open_subagent(1);
    assert_eq!(
        entry_kinds(&rig.app),
        [EntryKind::User, EntryKind::Assistant]
    );
    // The rows were replaced first, then the selection was set on them.
    let first = assert_follows(&rig.app, "the subagent");
    assert!(first <= 1, "one of its two entries: {first}");
    rig.press(Key::Char('G'));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "G in the subagent"), 1);
    rig.press(Key::Char('g'));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "g in the subagent"), 0);
    rig.press(Key::Char('G'));
    rig.settle();

    rig.back_to(0);
    assert_eq!(section(&rig.app), parent_section, "the parent as it was");
    assert_eq!(sidebar_rows(&rig.app), parent_rows);
    assert_eq!(sidebar_entry(&rig.app), Some(parent_entry));
    assert_follows(&rig.app, "back in the parent");
}

#[test]
fn a_nested_subagent_and_each_way_back_follow_the_transcript_on_top() {
    let mut rig = subagent_rig(120, 40);
    let parent_entry = sidebar_entry(&rig.app);
    rig.open_subagent(1);
    rig.press(Key::Char('G'));
    rig.press(Key::Char('l'));
    rig.settle();
    let first = assert_follows(&rig.app, "the first subagent");
    rig.open_subagent(2);
    assert_eq!(assert_follows(&rig.app, "the second subagent"), 0);
    rig.back_to(1);
    assert_eq!(sidebar_entry(&rig.app), Some(first));
    assert_follows(&rig.app, "back in the first");
    rig.back_to(0);
    assert_eq!(sidebar_entry(&rig.app), parent_entry);
}

// -- live updates

#[test]
fn a_turn_appended_while_away_from_the_end_adds_a_row_and_changes_neither_selection_nor_offset() {
    // 14 list rows: far fewer than the 43 the list has.
    let mut rig = long_rig(10, 100, 24);
    for _ in 0..9 {
        rig.press(Key::Char(']'));
    }
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "away from the end"), 9);
    let before = pane_rows(&mut rig, 100, 24);
    let marker = marker_row(&mut rig, 100, 24).expect("the marker is shown");
    assert!(marker > 0, "scrolled: the marker is inside the list");
    let selection = section(&rig.app);
    let entries = entry_kinds(&rig.app).len();
    let area = rig.app.routing().area(Id::TurnSidebar);

    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(reply("late", "A late reply."))]),
    ));

    assert_eq!(entry_kinds(&rig.app).len(), entries + 1, "a row was added");
    assert_eq!(section(&rig.app), selection);
    assert_eq!(sidebar_entry(&rig.app), Some(9));
    assert_eq!(
        pane_rows(&mut rig, 100, 24),
        before,
        "the pane did not move"
    );
    assert_eq!(marker_row(&mut rig, 100, 24), Some(marker));
    assert_eq!(rig.app.routing().area(Id::TurnSidebar), area);
}

#[test]
fn a_following_transcripts_selection_is_tracked_as_turns_arrive() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char('G'));
    rig.settle();
    let last = assert_follows(&rig.app, "at the end");
    assert_eq!(last, 7);
    for (n, id) in ["late1", "late2"].into_iter().enumerate() {
        rig.app.update(session_msg(
            &rig.app,
            ModelUpdate::Appended(vec![arc(reply(id, "Another reply."))]),
        ));
        assert_eq!(
            assert_follows(&rig.app, "after an append"),
            last + n + 1,
            "the new last turn"
        );
        assert_eq!(entry_kinds(&rig.app).len(), 9 + n);
    }
}

#[test]
fn a_new_turn_the_selection_is_not_on_changes_nothing_in_the_selection() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char(']'));
    rig.settle();
    let selection = section(&rig.app);
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(prompt("late", "One more question."))]),
    ));
    assert_eq!(section(&rig.app), selection);
    assert_eq!(sidebar_entry(&rig.app), Some(1));
    assert_eq!(entry_kinds(&rig.app).len(), 9);
}

// -- staying visible

#[test]
fn the_selected_row_stays_visible_and_the_pane_scrolls_by_the_least_needed() {
    // The pane shows 14 list rows, that is the rows 0..=13, of 83.
    let mut rig = long_rig(20, 100, 24);
    let area = rig.app.routing().area(Id::TurnSidebar).unwrap();
    assert_eq!(area, Rect::new(0, 5, 30, 17));
    let mut at = marker_row(&mut rig, 100, 24).expect("the first entry is shown");
    assert_eq!(at, 2, "the first entry is on the third row");
    let mut scrolled = 0;
    for n in 1..30 {
        rig.press(Key::Char(']'));
        rig.settle();
        assert_eq!(assert_follows(&rig.app, "down"), n);
        let now = marker_row(&mut rig, 100, 24).unwrap_or_else(|| panic!("hidden at {n}"));
        // One entry is two rows: the marker moves two rows down until the
        // last row, where the list scrolls and the marker stays.
        assert_eq!(now, (at + 2).min(13), "entry {n}");
        scrolled += usize::from(at + 2 > 13);
        at = now;
        assert_eq!(rig.app.routing().area(Id::TurnSidebar), Some(area));
    }
    assert_eq!(
        scrolled,
        29 - 5,
        "scrolled once per step after the pane filled"
    );
    for n in (0..29).rev() {
        rig.press(Key::Char('['));
        rig.settle();
        assert_eq!(assert_follows(&rig.app, "up"), n);
        let now = marker_row(&mut rig, 100, 24).unwrap_or_else(|| panic!("hidden at {n}"));
        // Up, the marker moves two rows up until the first row, where the
        // list scrolls and the marker stays.
        assert_eq!(now, at.saturating_sub(2), "entry {n}");
        at = now;
    }
    // Only as far as needed: the first entry is on the pane's first list
    // row, `Start` and the connector stay scrolled out.
    assert_eq!(at, 0, "scrolled up no further than the selected row");
    assert_eq!(rig.app.routing().area(Id::TurnSidebar), Some(area));
}

// -- a turn without an entry

#[test]
fn a_selection_in_a_turn_without_an_entry_leaves_the_sidebar_unchanged() {
    use crate::components::turn_block::fixtures as blocks;
    let system_only = crate::components::transcript::fixtures::with_id(
        blocks::user(
            "",
            vec![blocks::injected("i1", &[("file", "/notes.md\nUse it.")])],
        ),
        "s1",
    );
    let mut rig = rig();
    rig.open_turns(vec![
        arc(prompt("u1", "First.")),
        arc(system_only),
        arc(prompt("u2", "Second.")),
    ]);
    // System sections are shown by default: the middle turn has a section
    // but no entry.
    assert_eq!(
        entry_kinds(&rig.app).len(),
        2,
        "no entry for the system turn"
    );
    assert_eq!(sidebar_entry(&rig.app), Some(0));
    rig.press(Key::Char('j'));
    rig.settle();
    let in_system = section(&rig.app).expect("a selection");
    assert_eq!(in_system.turn.as_str(), "s1");
    assert_eq!(followed_entry(&rig.app), None);
    assert_eq!(sidebar_entry(&rig.app), Some(0), "it did not clear or jump");
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "past it"), 1);
    // Back through it from below.
    rig.press(Key::Char('k'));
    rig.settle();
    assert_eq!(section(&rig.app).unwrap().turn.as_str(), "s1");
    assert_eq!(
        sidebar_entry(&rig.app),
        Some(1),
        "it stays on the turn below"
    );
}

// -- no message, only on a change

#[test]
fn pushing_the_selection_produces_no_message() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.take_queued();
    // A key moves the selection; the pushing happens in `update`.
    rig.press(Key::Char(']'));
    rig.settle();
    assert_eq!(sidebar_entry(&rig.app), Some(1));
    rig.app.update(Msg::Transcript(TranscriptMsg::Changed));
    assert_eq!(rig.app.take_queued(), []);
    // A live update that moves the following selection.
    rig.press(Key::Char('G'));
    rig.settle();
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(reply("late", "Late."))]),
    ));
    assert_eq!(sidebar_entry(&rig.app), Some(8));
    assert_eq!(rig.app.take_queued(), []);
}

#[test]
fn the_app_takes_the_selection_from_the_transcripts_report_not_after_every_message() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char(']'));
    rig.settle();
    let before = assert_follows(&rig.app, "after the key");
    // Move the transcript's selection behind the App's back: no report.
    let top = rig.app.top_transcript();
    let component = rig.app.application_mut().get_component_mut(&top).unwrap();
    component.perform(Cmd::Custom(crate::components::transcript::NEXT_TURN));
    let moved = section(&rig.app).expect("a selection");
    assert_ne!(followed_entry(&rig.app), Some(before), "it moved");
    // A tick, a redraw request and a prompt's cursor ask the transcript
    // nothing: the sidebar stays.
    rig.app.update(Msg::Tick);
    rig.app.update(Msg::Transcript(TranscriptMsg::Changed));
    rig.app.update(Msg::Prompt(PromptMsg::Moved));
    assert_eq!(sidebar_entry(&rig.app), Some(before));
    // The report is what it follows.
    rig.app
        .update(Msg::Transcript(TranscriptMsg::SelectionChanged(moved)));
    assert_follows(&rig.app, "after the report");
}

/// The selected section of the transcript `id`, on top or covered.
fn section_of(app: &App, id: Id) -> Option<SectionId> {
    let value = app
        .application()
        .query(&id, SELECTED_SECTION_ATTR)
        .unwrap()?
        .into_attr();
    payload_from_value(&value)
}

#[test]
fn a_live_update_that_removes_the_selected_section_moves_the_sidebar_with_the_selection() {
    let mut rig = long_rig(3, 120, 40);
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "on a0"), 1);
    // `a0` is rewritten without the section that is selected: the
    // transcript moves its selection, which the App reads after the push.
    let changed = crate::components::transcript::fixtures::with_id(
        crate::components::turn_block::fixtures::assistant(
            None,
            vec![crate::components::turn_block::fixtures::text(
                "t2", "Changed.",
            )],
            None,
        ),
        "a0",
    );
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Updated(vec![arc(changed)]),
    ));
    let now = assert_follows(&rig.app, "after the update");
    assert_ne!(now, 1, "the selection left a0: {:?}", section(&rig.app));
}

#[test]
fn a_reset_and_the_same_turns_again_bring_the_sidebar_back_to_the_selection() {
    let mut rig = rig();
    rig.open_loaded();
    let first = assert_follows(&rig.app, "at the start");
    // The reset empties the sidebar's rows and the transcript's selection;
    // the selection the turns bring back is the same section as before.
    rig.app.update(session_msg(&rig.app, ModelUpdate::Reset));
    rig.app
        .update(session_msg(&rig.app, ModelUpdate::Appended(turns())));
    rig.app.update(session_msg(&rig.app, ModelUpdate::Loaded));
    assert_eq!(assert_follows(&rig.app, "after the reload"), first);
}

#[test]
fn closing_a_subagent_follows_the_parents_selection_that_moved_while_it_was_covered() {
    let mut rig = subagent_rig(120, 40);
    rig.press(Key::Char('G'));
    rig.settle();
    let before = assert_follows(&rig.app, "at the end");
    rig.app.update(Msg::Browser(BrowserMsg::OpenSubagent {
        call_id: crate::model::ItemId::from("call-a"),
        label: "story-writer".to_owned(),
    }));
    rig.pump_until(|app| app.subagent_depth() == 1 && app.session_loaded());
    rig.settle();
    // The covered parent follows its end: a late turn moves its selection.
    let covered_before = section_of(&rig.app, Id::Transcript);
    rig.harness.send(
        0,
        ModelUpdate::Appended(vec![arc(reply("late", "A late reply of the parent."))]),
    );
    rig.settle();
    assert_ne!(
        section_of(&rig.app, Id::Transcript),
        covered_before,
        "the covered parent's selection moved"
    );
    rig.back_to(0);
    let after = assert_follows(&rig.app, "back in the parent");
    assert_eq!(after, before + 1);
}

#[test]
fn the_selection_is_pushed_only_when_the_transcripts_section_changed() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char(']'));
    rig.settle();
    // Move the sidebar on its own, as a wheel over it will.
    rig.app
        .application_mut()
        .attr(
            &Id::TurnSidebar,
            SELECTED_ATTR,
            crate::components::selected_value(Some(6)),
        )
        .unwrap();
    assert_eq!(sidebar_entry(&rig.app), Some(6));
    // Messages that do not move the transcript's selection leave it be.
    rig.app.update(Msg::Transcript(TranscriptMsg::Changed));
    rig.app.update(Msg::Tick);
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(prompt("late", "Late."))]),
    ));
    assert_eq!(sidebar_entry(&rig.app), Some(6));
    // The next change of the transcript's section is followed.
    rig.press(Key::Char(']'));
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "after the next move"), 2);
}

#[test]
fn the_sidebar_does_not_follow_while_it_has_the_focus() {
    let mut rig = rig();
    rig.open_loaded();
    // The transcript follows the end: an append moves its selection.
    rig.press(Key::Char('G'));
    rig.settle();
    let section_before = section(&rig.app);
    assert_eq!(sidebar_entry(&rig.app), Some(7));
    rig.app.application_mut().active(&Id::TurnSidebar).unwrap();
    rig.app.update(session_msg(
        &rig.app,
        ModelUpdate::Appended(vec![arc(reply("late", "Late."))]),
    ));
    assert_ne!(section(&rig.app), section_before, "the transcript moved");
    assert_eq!(rig.focus(), Some(Id::TurnSidebar));
    assert_eq!(sidebar_entry(&rig.app), Some(7), "the sidebar stayed");
    assert_eq!(entry_kinds(&rig.app).len(), 9, "it got the row");
}

#[test]
fn a_turn_pushed_by_attribute_selects_its_row_and_an_unknown_turn_changes_nothing() {
    let mut rig = rig();
    rig.open_loaded();
    let rows = sidebar_rows(&rig.app);
    let (third, _) = rows.entry(2).unwrap();
    rig.app
        .application_mut()
        .attr(
            &Id::TurnSidebar,
            SELECTED_TURN_ATTR,
            payload_value(third.clone()),
        )
        .unwrap();
    assert_eq!(sidebar_entry(&rig.app), Some(2));
    rig.app
        .application_mut()
        .attr(
            &Id::TurnSidebar,
            SELECTED_TURN_ATTR,
            payload_value(crate::model::TurnId::from("nope")),
        )
        .unwrap();
    assert_eq!(sidebar_entry(&rig.app), Some(2));
}

// -- the wheel

#[test]
fn the_transcripts_wheel_moves_the_sidebar_selection_when_it_moves_the_transcripts() {
    let mut rig = long_rig(10, 120, 20);
    let start = section(&rig.app);
    assert_eq!(assert_follows(&rig.app, "start"), 0);
    let mut moved = Vec::new();
    for _ in 0..12 {
        rig.wheel(MouseEventKind::ScrollDown);
        moved.push(assert_follows(&rig.app, "wheel down"));
    }
    assert_ne!(section(&rig.app), start, "the wheel moved the selection");
    assert!(moved.last() > Some(&0), "{moved:?}");
    assert!(moved.windows(2).all(|w| w[0] <= w[1]), "{moved:?}");
    let down = *moved.last().unwrap();
    for _ in 0..12 {
        rig.wheel(MouseEventKind::ScrollUp);
        assert_follows(&rig.app, "wheel up");
    }
    assert!(sidebar_entry(&rig.app) < Some(down));
}

// -- snapshots (120x40, default theme)

#[test]
fn the_selection_on_an_assistant_row_120x40_text() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char(']'));
    rig.settle();
    assert_eq!(sidebar_entry(&rig.app), Some(1));
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn the_selection_on_an_assistant_row_120x40_styled() {
    let mut rig = rig();
    rig.open_loaded();
    rig.press(Key::Char(']'));
    rig.settle();
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn the_selection_on_a_compaction_row_120x40_text() {
    let mut rig = rig();
    rig.open_loaded();
    for _ in 0..4 {
        rig.press(Key::Char(']'));
    }
    rig.settle();
    assert_eq!(sidebar_entry(&rig.app), Some(4));
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn the_selection_on_a_compaction_row_120x40_styled() {
    let mut rig = rig();
    rig.open_loaded();
    for _ in 0..4 {
        rig.press(Key::Char(']'));
    }
    rig.settle();
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn a_long_list_scrolled_to_keep_the_selected_row_visible_120x40_text() {
    // 40 entries: 83 rows against the 30 the pane shows.
    let mut rig = long_rig(20, 120, 40);
    for _ in 0..22 {
        rig.press(Key::Char(']'));
    }
    rig.settle();
    assert_eq!(assert_follows(&rig.app, "scrolled"), 22);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn a_long_list_scrolled_to_keep_the_selected_row_visible_120x40_styled() {
    let mut rig = long_rig(20, 120, 40);
    for _ in 0..22 {
        rig.press(Key::Char(']'));
    }
    rig.settle();
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

// -- going back from a subagent

#[test]
fn going_back_from_a_subagent_shows_the_pane_as_it_was_with_a_list_longer_than_the_pane() {
    // 30 list rows against 90 rows; the selected row is not on the last one.
    let mut rig = subagent_rig_after(long_turns(20), 120, 30);
    let _ = screen_text(&mut rig, 120, 40);
    let before = screen_text(&mut rig, 120, 40);
    let at = marker_row(&mut rig, 120, 40).expect("the selected row is shown");
    assert!(0 < at && at < 29, "away from both edges: {at}");
    let entry = sidebar_entry(&rig.app);

    rig.open_subagent(1);
    assert_eq!(entry_kinds(&rig.app).len(), 2);
    rig.back_to(0);

    assert_eq!(sidebar_entry(&rig.app), entry);
    assert_eq!(marker_row(&mut rig, 120, 40), Some(at));
    assert_eq!(
        screen_text(&mut rig, 120, 40),
        before,
        "the screen as it was"
    );
    insta::assert_snapshot!(before);
}

#[test]
fn going_back_scrolls_the_least_needed_when_the_pane_shrank_while_it_was_covered() {
    let mut rig = subagent_rig_after(long_turns(20), 120, 40);
    let entry = sidebar_entry(&rig.app);
    rig.open_subagent(1);
    let _ = screen_text(&mut rig, 120, 24);
    rig.back_to(0);
    assert_eq!(sidebar_entry(&rig.app), entry);
    assert_eq!(marker_row(&mut rig, 120, 24), Some(13), "on the last row");
}

#[test]
fn nested_subagents_bring_back_each_pane_as_it_was() {
    let mut rig = subagent_rig_after(long_turns(20), 120, 30);
    let _ = screen_text(&mut rig, 120, 40);
    let parent = screen_text(&mut rig, 120, 40);
    rig.open_subagent(1);
    rig.press(Key::Char('G'));
    rig.press(Key::Char('l'));
    rig.settle();
    let first = screen_text(&mut rig, 120, 40);
    rig.open_subagent(2);
    rig.back_to(1);
    assert_eq!(screen_text(&mut rig, 120, 40), first);
    rig.back_to(0);
    assert_eq!(screen_text(&mut rig, 120, 40), parent);
}
