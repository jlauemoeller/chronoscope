//! Notices in the transcript: selecting, folding keys, copying, saving,
//! exporting and searching them.

use tuirealm::component::Component;
use tuirealm::event::{Key, KeyModifiers};

use super::super::fixtures::*;
use super::super::*;
use super::keys::{ch, key, press};
use super::{item_section, section_id};
use crate::components::turn_block::fixtures as blocks;
use crate::components::turn_block::{SectionKey, SectionKind};
use crate::model::{ItemId, NoticeLevel};
use crate::transcript_search::{SearchPosition, SearchRequest};

const ERROR: &str = "provider.invalid-request: messages: text content blocks must be non-empty";
const WARNING: &str = "aborted: Step interrupted";

/// A prompt, an assistant turn that ends in an error notice, a prompt and an
/// assistant turn that ends in a warning notice.
fn notices() -> Transcript {
    let turns = vec![
        arc(prompt("u1", "Run the build")),
        arc(with_id(
            blocks::assistant(
                None,
                vec![
                    blocks::text("x1", "Starting."),
                    blocks::notice("n1", NoticeLevel::Error, ERROR),
                ],
                None,
            ),
            "a1",
        )),
        arc(prompt("u2", "Try again")),
        arc(with_id(
            blocks::assistant(
                None,
                vec![blocks::notice("n2", NoticeLevel::Warning, WARNING)],
                None,
            ),
            "a2",
        )),
    ];
    transcript_of(turns, 100, 30)
}

fn error() -> SectionId {
    item_section("a1", "n1")
}

fn warning() -> SectionId {
    item_section("a2", "n2")
}

fn on(id: &SectionId) -> Transcript {
    let mut t = notices();
    assert!(t.select(id), "{id:?} is selectable");
    assert_eq!(t.selected(), Some(id));
    t
}

fn copied(msg: Option<Msg>) -> String {
    match msg {
        Some(Msg::Output(OutputMsg::CopyText(text))) => text,
        other => panic!("expected CopyText, got {other:?}"),
    }
}

fn saved(msg: Option<Msg>) -> (String, &'static str) {
    match msg {
        Some(Msg::Output(OutputMsg::SaveSection { text, kind })) => (text, kind),
        other => panic!("expected SaveSection, got {other:?}"),
    }
}

fn exported(msg: Option<Msg>) -> String {
    match msg {
        Some(Msg::Output(OutputMsg::Export { document, .. })) => document,
        other => panic!("expected Export, got {other:?}"),
    }
}

#[test]
fn moving_through_the_transcript_stops_on_each_notice() {
    let mut t = notices();
    let mut stops = Vec::new();
    for _ in 0..12 {
        t.perform(super::down());
        if let Some(id) = t.selected() {
            stops.push(id.clone());
        }
    }
    assert!(stops.contains(&error()), "{stops:?}");
    assert!(stops.contains(&warning()), "{stops:?}");
    // In the turn's order: after the text, before the next prompt.
    let at = |id: &SectionId| stops.iter().position(|s| s == id).unwrap();
    assert!(at(&item_section("a1", "x1")) < at(&error()));
    assert!(at(&error()) < at(&section_id("u2", SectionKey::Prompt)));
}

#[test]
fn a_notice_is_not_foldable_and_has_no_marker() {
    let mut t = on(&error());
    let before = text_rows(&draw(&mut t, 100, 30));
    for k in [Key::Enter, Key::Left, Key::Right] {
        press(&mut t, k);
    }
    for c in ['h', 'l', ' '] {
        ch(&mut t, c);
    }
    assert_eq!(t.is_folded(&error()), None);
    assert_eq!(
        t.perform(Cmd::Custom(FOLD)),
        CmdResult::NoChange,
        "the fold command"
    );
    assert_eq!(text_rows(&draw(&mut t, 100, 30)), before);
    let row = before
        .iter()
        .find(|row| row.contains(ERROR))
        .expect("the notice is drawn");
    assert!(!row.contains('▶') && !row.contains('▼'), "{row}");
    assert!(row.contains(&format!("│   {ERROR}")) || row.contains(&format!("▒   {ERROR}")));
}

#[test]
fn y_copies_the_text_of_the_notice() {
    let mut t = on(&error());
    assert_eq!(copied(ch(&mut t, 'y')), ERROR);
    assert_eq!(copied(ch(&mut t, 'Y')), ERROR);
    let mut t = on(&warning());
    assert_eq!(copied(ch(&mut t, 'y')), WARNING);
    assert_eq!(copied(ch(&mut t, 'Y')), WARNING);
}

#[test]
fn a_copy_of_a_wrapped_notice_has_no_added_breaks() {
    let mut t = notices();
    t = t.viewport(ratatui::layout::Size::new(40, 30));
    assert!(t.select(&error()));
    assert_eq!(copied(ch(&mut t, 'y')), ERROR);
}

#[test]
fn w_saves_a_notice_under_error_or_notice() {
    let mut t = on(&error());
    assert_eq!(saved(ch(&mut t, 'w')), (ERROR.to_owned(), "error"));
    assert_eq!(
        saved(t.on(&key(Key::Char('W'), KeyModifiers::SHIFT))),
        (ERROR.to_owned(), "error")
    );
    let mut t = on(&warning());
    assert_eq!(saved(ch(&mut t, 'w')), (WARNING.to_owned(), "notice"));
    assert_eq!(
        saved(t.on(&key(Key::Char('W'), KeyModifiers::SHIFT))),
        (WARNING.to_owned(), "notice")
    );
    assert_eq!(t.file_kind(), Some("notice"));
}

#[test]
fn both_exports_write_a_notice_like_the_model_note() {
    let mut t = notices();
    let folded = exported(ch(&mut t, 'e'));
    let full = exported(t.on(&key(Key::Char('E'), KeyModifiers::SHIFT)));
    for document in [&folded, &full] {
        assert!(document.contains(&format!("\n_{ERROR}_\n")), "{document}");
        assert!(document.contains(&format!("\n_{WARNING}_\n")), "{document}");
    }
}

fn exported_notice(text: &str) -> String {
    let turns = vec![arc(with_id(
        blocks::assistant(
            None,
            vec![blocks::notice("n1", NoticeLevel::Error, text)],
            None,
        ),
        "a1",
    ))];
    transcript_of(turns, 100, 30).export_document(true)
}

#[test]
fn a_notice_of_several_paragraphs_is_emphasised_per_line() {
    assert!(
        exported_notice("first line\n\nsecond paragraph")
            .contains("\n_first line_\n\n_second paragraph_\n")
    );
}

#[test]
fn an_exported_notice_keeps_its_indent_outside_the_emphasis_and_escapes_markers() {
    let document = exported_notice("one\n  two_with_underscore *star*");
    assert!(
        document.contains("\n_one_\n  _two\\_with\\_underscore \\*star\\*_\n"),
        "{document}"
    );
}

fn position(t: &Transcript) -> Option<SearchPosition> {
    let value = t.query(SEARCH_POSITION_ATTR).unwrap().into_attr();
    payload_from_value::<Option<SearchPosition>>(&value).unwrap()
}

fn search(t: &mut Transcript, query: &str) -> Option<String> {
    t.attr(
        SEARCH_QUERY_ATTR,
        search_start_value(SearchStart {
            query: query.to_owned(),
            generation: 1,
        }),
    );
    let value = t.query(SEARCH_REQUEST_ATTR).unwrap().into_attr();
    let request = payload_from_value::<SearchRequest>(&value).expect("a request");
    t.attr(SEARCH_RESULTS_ATTR, search_results_value(request.run()));
    position(t).map(|p| p.to_string())
}

#[test]
fn a_search_finds_the_text_of_a_notice_and_jumps_to_it() {
    let mut t = notices();
    assert_eq!(search(&mut t, "interrupted").as_deref(), Some("1/1"));
    assert_eq!(t.selected(), Some(&warning()));
    assert_eq!(search(&mut t, "invalid-request").as_deref(), Some("1/1"));
    assert_eq!(t.selected(), Some(&error()));
    // Only the warning contains `aborted`.
    assert_eq!(search(&mut t, "aborted").as_deref(), Some("1/1"));
    assert_eq!(t.selected(), Some(&warning()));
}

#[test]
fn the_match_in_a_notice_is_highlighted_with_search_match() {
    let mut t = notices();
    assert_eq!(search(&mut t, "interrupted").as_deref(), Some("1/1"));
    let bg = blocks::theme().role(crate::theme::Role::Primary);
    let buf = draw(&mut t, 100, 30);
    let mut found = String::new();
    for y in 0..30 {
        for x in 0..100 {
            let cell = &buf[(x, y)];
            if cell.bg == bg && cell.symbol() != " " {
                found.push_str(cell.symbol());
            }
        }
    }
    assert_eq!(found, "interrupted");
}

#[test]
fn a_notice_has_a_section_of_its_own_in_every_turn_it_is_in() {
    let t = notices();
    let ids: Vec<ItemId> = (0..t.turn_count())
        .flat_map(|i| {
            crate::components::turn_block::derive_sections(t.turn(i).unwrap(), true)
                .into_iter()
                .filter_map(|s| match s.id.key {
                    SectionKey::Item(id) if s.kind() == SectionKind::Notice => Some(id),
                    _ => None,
                })
        })
        .collect();
    assert_eq!(ids, [ItemId::from("n1"), ItemId::from("n2")]);
}

#[test]
fn a_hyphenated_word_is_not_split_across_rows_so_a_search_highlights_it() {
    let mut t = notices();
    assert_eq!(search(&mut t, "non-empty").as_deref(), Some("1/1"));
    let bg = blocks::theme().role(crate::theme::Role::Primary);
    let buf = draw(&mut t, 100, 30);
    let mut found = String::new();
    for y in 0..30 {
        for x in 0..100 {
            let cell = &buf[(x, y)];
            if cell.bg == bg && cell.symbol() != " " {
                found.push_str(cell.symbol());
            }
        }
    }
    assert_eq!(found, "non-empty");
    // Narrow enough to wrap inside the message: no row ends in a hyphen.
    let mut t = notices().viewport(ratatui::layout::Size::new(40, 40));
    let rows = text_rows(&draw(&mut t, 40, 40));
    assert!(
        rows.iter()
            .all(|r| !r.trim_end_matches([' ', '│', '▒']).ends_with('-')),
        "{rows:#?}"
    );
}

#[test]
fn a_control_character_in_a_notice_is_searched_as_the_space_it_is_drawn_as() {
    let turns = vec![arc(with_id(
        blocks::assistant(
            None,
            vec![blocks::notice(
                "n1",
                NoticeLevel::Error,
                "alpha\tbeta and more",
            )],
            None,
        ),
        "a1",
    ))];
    let mut t = transcript_of(turns, 100, 30);
    assert_eq!(search(&mut t, "alpha beta").as_deref(), Some("1/1"));
}

#[test]
fn trailing_whitespace_of_a_notice_draws_no_extra_marked_row() {
    let turns = vec![arc(with_id(
        blocks::assistant(
            None,
            vec![blocks::notice(
                "n1",
                NoticeLevel::Error,
                "first\nsecond\n\n",
            )],
            None,
        ),
        "a1",
    ))];
    let mut t = transcript_of(turns, 60, 20);
    assert!(t.select(&error()) || t.select(&item_section("a1", "n1")));
    let rows = text_rows(&draw(&mut t, 60, 20));
    let marked: Vec<&String> = rows.iter().filter(|r| r.starts_with('▒')).collect();
    assert_eq!(marked.len(), 2, "{rows:#?}");
    assert!(
        marked
            .iter()
            .all(|r| !r.trim_matches(['▒', '│', ' ']).is_empty())
    );
    assert_eq!(
        copied(ch(&mut t, 'y')),
        "first\nsecond",
        "nothing trailing is copied"
    );
}
