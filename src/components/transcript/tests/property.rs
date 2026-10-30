//! Property tests: cached heights equal what the blocks actually draw.

use proptest::prelude::*;
use serde_json::json;

use super::super::fixtures::*;
use super::super::*;
use crate::components::turn_block::derive_sections;
use crate::components::turn_block::fixtures as blocks;
use crate::model::{Thinking, ToolKind, ToolStatus, Turn};
use crate::test_support::render;

/// What an item of a generated assistant turn is.
#[derive(Debug, Clone)]
enum ItemSpec {
    Text(String),
    Thinking(String),
    Tool {
        /// An index into `KINDS`.
        kind: usize,
        description: String,
        command: String,
        output: Option<String>,
        is_error: bool,
    },
    Injected(String),
    Model,
}

#[derive(Debug, Clone)]
enum TurnSpec {
    User { text: String, injected: Vec<String> },
    Assistant { items: Vec<ItemSpec>, stats: bool },
    Compaction(String),
}

fn text() -> impl Strategy<Value = String> {
    "([a-zA-Z0-9 .,`*_#>\n-]|日|é){0,300}"
}

/// Every kind of tool call, so that every body renderer is measured.
fn kind_of(index: usize) -> ToolKind {
    match index % 10 {
        0 => ToolKind::Shell,
        1 => ToolKind::Read,
        2 => ToolKind::Write,
        3 => ToolKind::Edit,
        4 => ToolKind::Grep,
        5 => ToolKind::Glob,
        6 => ToolKind::WebFetch,
        7 => ToolKind::WebSearch,
        8 => ToolKind::Subagent,
        _ => ToolKind::Other("mcp__x__y".to_owned()),
    }
}

/// An input that has the fields of every kind, so each renderer finds its own.
fn input_of(text: &str) -> serde_json::Value {
    json!({
        "command": text,
        "file_path": format!("/src/{}.rs", text.len()),
        "offset": text.len() % 7,
        "pattern": text,
        "path": "/work",
        "url": text,
        "prompt": text,
        "query": text,
        "content": text,
        "subagent_type": "Explore",
        "old_string": text,
        "new_string": text.to_uppercase(),
    })
}

fn item() -> impl Strategy<Value = ItemSpec> {
    prop_oneof![
        text().prop_map(ItemSpec::Text),
        text().prop_map(ItemSpec::Thinking),
        (
            0usize..10,
            text(),
            text(),
            proptest::option::of(prop_oneof![text(), "(1[0-9]\n|é\n|日本\n){0,30}"]),
            any::<bool>()
        )
            .prop_map(
                |(kind, description, command, output, is_error)| ItemSpec::Tool {
                    kind,
                    description,
                    command,
                    output,
                    is_error,
                }
            ),
        text().prop_map(ItemSpec::Injected),
        Just(ItemSpec::Model),
    ]
}

fn turn_spec() -> impl Strategy<Value = TurnSpec> {
    prop_oneof![
        (text(), proptest::collection::vec(text(), 0..2))
            .prop_map(|(text, injected)| TurnSpec::User { text, injected }),
        (proptest::collection::vec(item(), 0..5), any::<bool>())
            .prop_map(|(items, stats)| TurnSpec::Assistant { items, stats }),
        text().prop_map(TurnSpec::Compaction),
    ]
}

fn build_item(n: usize, spec: &ItemSpec) -> crate::model::Item {
    let id = format!("i{n}");
    match spec {
        ItemSpec::Text(t) => blocks::text(&id, t),
        ItemSpec::Thinking(t) => blocks::thinking(&id, Thinking::Text(t.clone())),
        ItemSpec::Tool {
            kind,
            description,
            command,
            output,
            is_error,
        } => blocks::tool_of(
            kind_of(*kind),
            &id,
            description,
            input_of(command),
            output.as_deref().map(|o| (o, *is_error)),
            if *is_error {
                ToolStatus::Error
            } else {
                ToolStatus::Ok
            },
        ),
        ItemSpec::Injected(t) => blocks::injected(&id, &[("file", t)]),
        ItemSpec::Model => crate::model::Item::ModelChange {
            id: crate::model::ItemId::from(id.as_str()),
            model: "claude-sonnet-5.5".to_owned(),
        },
    }
}

fn build_turn(n: usize, spec: &TurnSpec) -> Turn {
    let id = format!("t{n}");
    let turn = match spec {
        TurnSpec::User { text, injected } => {
            let items = injected
                .iter()
                .enumerate()
                .map(|(k, t)| blocks::injected(&format!("j{k}"), &[("file", t)]))
                .collect();
            blocks::user(text, items)
        }
        TurnSpec::Assistant { items, stats } => {
            let items = items
                .iter()
                .enumerate()
                .map(|(k, s)| build_item(k, s))
                .collect();
            blocks::assistant(Some("high"), items, stats.then(blocks::stats))
        }
        TurnSpec::Compaction(summary) => blocks::compaction(summary),
    };
    with_id(turn, &id)
}

/// The number of rows a block draws, read off the screen: the row of the
/// bottom border, counted from the top, with nothing drawn below it.
fn rendered_height(t: &Transcript, index: usize, width: u16, expected: usize) -> usize {
    let content = &t.content;
    let block = content.block(index);
    let rows = u16::try_from(expected + 3).unwrap();
    let buf = render(width, rows, |frame| {
        ratatui::widgets::Widget::render(&block, frame.area(), frame.buffer_mut());
    });
    let text = blocks::rows(&buf);
    let bottom = text
        .iter()
        .position(|row| row.starts_with('╰'))
        .expect("a bottom border");
    assert!(text[bottom + 1..].iter().all(|row| row.trim().is_empty()));
    bottom + 1
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 48,
        failure_persistence: None,
        ..ProptestConfig::default()
    })]

    #[test]
    fn cached_heights_equal_rendered_heights(
        specs in proptest::collection::vec(turn_spec(), 1..6),
        width in 20u16..=200,
        height in 5u16..40,
        folds in proptest::collection::vec(any::<bool>(), 1..16),
        expands in proptest::collection::vec(any::<bool>(), 1..16),
        truncate in 1usize..8,
        offset in 0usize..400,
    ) {
        let turns: Vec<_> = specs.iter().enumerate().map(|(n, s)| arc(build_turn(n, s))).collect();
        let mut t = transcript_of(turns, width, height);
        // A small limit so that the generated outputs get truncated.
        t.attr(TRUNCATE_OUTPUT_LINES_ATTR, AttrValue::Length(truncate));
        let mut bit = 0;
        for index in 0..t.turn_count() {
            for section in derive_sections(&t.turn(index).unwrap().clone(), true) {
                if expands[bit % expands.len()] {
                    t.content.folds.set_expanded(section.id.clone());
                }
                t.set_folded(&section.id, folds[bit % folds.len()]);
                bit += 1;
            }
        }
        let n = t.turn_count();
        let cached: Vec<usize> = t.measured(|cache, m| (0..n).map(|i| cache.height(i, m)).collect());
        for (i, h) in cached.iter().enumerate() {
            prop_assert_eq!(*h, rendered_height(&t, i, width, *h), "block {}", i);
        }
        let total = t.measured(|cache, m| cache.total(m));
        prop_assert_eq!(total, cached.iter().sum::<usize>());

        // The selection stops (sections and truncation rows) are in order,
        // inside the block and do not overlap.
        for (i, h) in cached.iter().enumerate() {
            let rows = t.content.section_rows(t.turn(i).unwrap(), width);
            let mut next = 1;
            for (_, range) in &rows {
                prop_assert!(range.start >= next && range.end > range.start, "{:?}", rows);
                next = range.end;
            }
            prop_assert!(next < *h, "{:?} in {}", rows, h);
        }

        // The frame is the matching slice of the stacked blocks.
        t.scroll_to(offset);
        let all: Vec<String> = (0..n).flat_map(|i| block_rows(&t, i, width)).collect();
        prop_assert_eq!(all.len(), total);
        let screen = text_rows(&draw(&mut t, width, height));
        let start = t.offset();
        for (y, row) in screen.iter().enumerate() {
            let expected = all.get(start + y).map_or("", String::as_str);
            prop_assert_eq!(row.trim_end(), expected.trim_end(), "row {}", y);
        }
    }

    #[test]
    fn a_resize_leaves_heights_matching_the_new_width(
        specs in proptest::collection::vec(turn_spec(), 1..5),
        from in 20u16..=200,
        to in 20u16..=200,
    ) {
        let turns: Vec<_> = specs.iter().enumerate().map(|(n, s)| arc(build_turn(n, s))).collect();
        let mut t = transcript_of(turns, from, 12);
        t.set_viewport(Size::new(to, 12));
        let n = t.turn_count();
        let cached: Vec<usize> = t.measured(|cache, m| (0..n).map(|i| cache.height(i, m)).collect());
        for (i, h) in cached.iter().enumerate() {
            prop_assert_eq!(*h, rendered_height(&t, i, to, *h), "block {}", i);
        }
    }
}
