//! Going to a block by its turn (what the turns sidebar asks for), and the
//! keys that hand the focus to the sidebar. The scroll is the one `]` and `[`
//! use.

use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use super::keys::{ch, press};
use super::*;
use crate::app::{Msg, UserEvent};

fn go_to(t: &mut Transcript, turn: &str) {
    t.attr(GO_TO_TURN_ATTR, payload_value(TurnId::from(turn)));
}

fn tops(t: &mut Transcript) -> Vec<usize> {
    (0..t.turn_count())
        .map(|b| t.measured(|cache, m| cache.top(b, m)))
        .collect()
}

/// A prompt whose text renders no rows (a link reference definition: the
/// markdown renderer draws nothing for it). The turn has an entry in the
/// sidebar, but no section to select.
fn comment_only(id: &str) -> Arc<Turn> {
    arc(prompt(id, "[//]: # (nothing to see)"))
}

#[test]
fn going_to_a_turn_gives_the_result_of_the_turn_keys_for_every_block() {
    let reference = session_transcript(80, 24);
    for block in 1..reference.turn_count() {
        let id = format!("t{}", block + 1);
        // `]` from the block before it.
        let mut by_key = session_transcript(80, 24);
        assert!(by_key.select(&by_key.sections_of(block - 1)[0]));
        ch(&mut by_key, ']');
        // The same block by its turn, from the top.
        let mut by_turn = session_transcript(80, 24);
        go_to(&mut by_turn, &id);
        assert_eq!(
            by_turn.selected(),
            by_key.selected(),
            "the selection of block {block}"
        );
        assert_eq!(
            by_turn.offset(),
            by_key.offset(),
            "the offset of block {block}"
        );
        assert_eq!(by_turn.selected().unwrap().turn.as_str(), id);
    }
}

#[test]
fn the_blocks_top_border_is_the_top_row_or_as_near_as_the_end_allows() {
    let mut t = session_transcript(80, 24);
    let tops = tops(&mut t);
    let total = total_height(&mut t);
    for (block, top) in tops.iter().enumerate() {
        go_to(&mut t, &format!("t{}", block + 1));
        assert_eq!(t.offset(), (*top).min(total - 24), "block {block}");
        if *top + 24 <= total {
            let rows = text_rows(&draw(&mut t, 80, 24));
            assert!(rows[0].starts_with('╭'), "block {block}: {}", rows[0]);
        }
    }
}

#[test]
fn going_to_a_block_can_go_back_up_too() {
    let mut t = session_transcript(80, 24);
    go_to(&mut t, "t7");
    let down = t.offset();
    go_to(&mut t, "t2");
    let tops = tops(&mut t);
    assert!(t.offset() < down);
    assert_eq!(t.offset(), tops[1]);
    assert_eq!(t.selected().unwrap().turn, TurnId::from("t2"));
}

#[test]
fn a_block_that_renders_no_rows_is_scrolled_to_and_leaves_the_selection() {
    let mut turns = session();
    turns.insert(3, comment_only("hidden"));
    let mut t = transcript_of(turns, 80, 24);
    let at = t.content.index[&TurnId::from("hidden")];
    assert!(t.sections_of(at).is_empty(), "nothing is selectable in it");
    assert!(t.selected().is_some());
    let before = t.selected().cloned();
    go_to(&mut t, "hidden");
    assert_eq!(t.selected().cloned(), before, "the selection is as it was");
    let top = t.measured(|cache, m| cache.top(at, m));
    assert_eq!(t.offset(), top, "but the view is at the block");
}

#[test]
fn a_turn_the_transcript_does_not_hold_changes_nothing() {
    let mut t = session_transcript(80, 24);
    go_to(&mut t, "t4");
    let (offset, selected) = (t.offset(), t.selected().cloned());
    go_to(&mut t, "no-such-turn");
    assert_eq!((t.offset(), t.selected().cloned()), (offset, selected));
}

#[test]
fn going_to_a_block_away_from_the_end_stops_following_and_counts_new_sections() {
    let mut t = session_transcript(80, 24);
    t.perform(Cmd::GoTo(Position::End));
    assert!(t.following());
    go_to(&mut t, "t2");
    assert!(!t.following(), "left the end");
    t.attr(
        MODEL_UPDATE_ATTR,
        model_update_value(ModelUpdate::Appended(vec![arc(reply("late", "Late."))])),
    );
    assert_eq!(t.new_sections(), 1);
    let offset = t.offset();
    assert_eq!(offset, tops(&mut t)[1], "the view did not move");
}

#[test]
fn going_to_the_last_block_keeps_following_at_the_end() {
    let mut t = session_transcript(80, 24);
    t.perform(Cmd::GoTo(Position::End));
    go_to(&mut t, "t8");
    assert!(t.following(), "the end of the transcript is still in view");
    assert_eq!(t.new_sections(), 0);
}

#[test]
fn going_to_a_block_before_a_viewport_is_announced_selects_it() {
    let mut t = Transcript::new(blocks::theme(), blocks::formatter()).with_turns(session());
    go_to(&mut t, "t3");
    assert_eq!(t.selected().unwrap().turn, TurnId::from("t3"));
    assert_eq!(t.offset(), 0);
}

#[test]
fn the_attribute_is_set_only() {
    let mut t = session_transcript(80, 24);
    go_to(&mut t, "t3");
    assert!(t.query(GO_TO_TURN_ATTR).is_none());
}

// -- the keys that focus the sidebar

fn shift_tab() -> Event<UserEvent> {
    Event::Keyboard(KeyEvent::new(Key::BackTab, KeyModifiers::SHIFT))
}

#[test]
fn one_tab_and_shift_tab_ask_for_the_sidebar_and_move_nothing() {
    let mut t = session_transcript(80, 24);
    ch(&mut t, 'j');
    let before = t.presentation();
    assert_eq!(ch(&mut t, '1'), Some(Msg::FocusTurns));
    assert_eq!(press(&mut t, Key::Tab), Some(Msg::FocusTurns));
    assert_eq!(t.on(&shift_tab()), Some(Msg::FocusTurns));
    assert_eq!(t.presentation(), before);
}

#[test]
fn the_focus_keys_follow_the_keymap() {
    let mut raw = crate::config::RawKeys::new();
    let browser = raw.entry("browser".to_owned()).or_default();
    browser.insert("focus_turns".to_owned(), vec!["f".to_owned()]);
    browser.insert("next_pane".to_owned(), vec!["ctrl-n".to_owned()]);
    let (keymap, warnings) = crate::keymap::Keymap::from_config(&raw);
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut t = session_transcript(80, 24).keymap(keymap);
    assert_eq!(ch(&mut t, 'f'), Some(Msg::FocusTurns));
    assert_eq!(
        t.on(&super::keys::key(Key::Char('n'), KeyModifiers::CONTROL)),
        Some(Msg::FocusTurns)
    );
    assert_eq!(ch(&mut t, '1'), None, "the default is replaced");
    assert_eq!(press(&mut t, Key::Tab), None);
}
