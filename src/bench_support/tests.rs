//! Tests of the generator and of the measurement helpers.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::sync::Arc;

use ratatui::layout::Size;
use tuirealm::component::Component;

use super::*;
use crate::harness::claude::Assembler;
use crate::model::{Item, ModelUpdate, Thinking, ToolKind, Turn, source_reads, text_calls};
use crate::test_support::render;

const MIB: u64 = 1024 * 1024;

fn generated(dir: &Path, bytes: u64, seed: u64) -> SyntheticSession {
    generate_session(&dir.join(format!("{SESSION_ID}.jsonl")), bytes, seed).unwrap()
}

#[test]
fn the_same_seed_gives_the_same_bytes_and_another_seed_other_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let a = generated(&tmp.path().join("a"), MIB, 1);
    let b = generated(&tmp.path().join("b"), MIB, 1);
    let c = generated(&tmp.path().join("c"), MIB, 2);
    assert_eq!(fs::read(&a.path).unwrap(), fs::read(&b.path).unwrap());
    assert_ne!(fs::read(&a.path).unwrap(), fs::read(&c.path).unwrap());
    assert_eq!((a.lines, a.tool_calls), (b.lines, b.tool_calls));
}

#[test]
fn the_file_has_the_requested_size() {
    let tmp = tempfile::tempdir().unwrap();
    for target in [MIB, 3 * MIB] {
        let session = generated(tmp.path(), target, 7);
        let on_disk = fs::metadata(&session.path).unwrap().len();
        assert_eq!(session.bytes, on_disk);
        assert!(on_disk >= target, "{on_disk} < {target}");
        assert!(on_disk < target + MIB, "{on_disk} is far above {target}");
    }
}

#[test]
fn every_line_is_json_and_the_mix_is_all_there() {
    let tmp = tempfile::tempdir().unwrap();
    let session = generated(tmp.path(), 5 * MIB, SEED);
    let text = fs::read_to_string(&session.path).unwrap();
    assert!(text.ends_with('\n'));
    let mut types = BTreeSet::new();
    let mut tools = BTreeSet::new();
    let mut blocks = BTreeSet::new();
    let mut attachments = BTreeSet::new();
    let mut multiline_prompt = false;
    let mut large = Vec::new();
    let mut lines = 0;
    for line in text.lines() {
        lines += 1;
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        types.insert(value["type"].as_str().unwrap().to_owned());
        if let Some(kind) = value.pointer("/attachment/type").and_then(|v| v.as_str()) {
            attachments.insert(kind.to_owned());
        }
        if value["subtype"] == "turn_duration" {
            types.insert("turn_duration".to_owned());
        }
        if value.pointer("/origin/kind").is_some_and(|k| k == "human") {
            multiline_prompt |= value
                .pointer("/message/content")
                .and_then(|c| c.as_str())
                .is_some_and(|c| c.contains('\n'));
        }
        for block in value
            .pointer("/message/content")
            .and_then(|c| c.as_array())
            .into_iter()
            .flatten()
        {
            let kind = block["type"].as_str().unwrap();
            blocks.insert(kind.to_owned());
            if kind == "tool_use" {
                tools.insert(block["name"].as_str().unwrap().to_owned());
            }
            if kind == "thinking" {
                let readable = !block["thinking"].as_str().unwrap().is_empty();
                blocks.insert(format!("thinking readable={readable}"));
            }
            if kind == "tool_result" {
                let size = block["content"].as_str().unwrap().len();
                if size >= LARGE_MIN {
                    assert!(size <= LARGE_MAX, "{size}");
                    large.push(size);
                }
            }
        }
    }
    assert_eq!(lines, session.lines);
    for kind in ["user", "assistant", "attachment", "system", "turn_duration"] {
        assert!(types.contains(kind), "no {kind} entry in {types:?}");
    }
    assert_eq!(
        tools,
        ["Bash", "Edit", "Grep", "Read"].map(str::to_owned).into()
    );
    for kind in [
        "text",
        "thinking",
        "tool_use",
        "tool_result",
        "thinking readable=true",
        "thinking readable=false",
    ] {
        assert!(blocks.contains(kind), "no {kind} block in {blocks:?}");
    }
    assert!(multiline_prompt, "no multi-line human prompt");
    assert!(attachments.len() >= 2, "{attachments:?}");
    assert!(!large.is_empty(), "no large output in 5 MiB");
    assert_eq!(large.len(), session.large_outputs);
    // About 5% of the results are large; the sample is small, so the bound
    // is loose.
    let results = text.matches(r#""type":"tool_result""#).count();
    assert!(
        large.len() * 100 <= results * 15,
        "{} of {results}",
        large.len()
    );
}

#[test]
fn every_generated_tool_result_carries_a_tool_use_result_that_repeats_its_output() {
    // The budgets cover the `toolUseResult` path only if the fixture has it.
    let tmp = tempfile::tempdir().unwrap();
    let session = generated(tmp.path(), 2 * MIB, SEED);
    let text = fs::read_to_string(&session.path).unwrap();
    let (mut objects, mut strings) = (0, 0);
    for line in text.lines() {
        let value: serde_json::Value = serde_json::from_str(line).unwrap();
        let Some(block) = value
            .pointer("/message/content/0")
            .filter(|b| b["type"] == "tool_result")
        else {
            continue;
        };
        let output = block["content"].as_str().unwrap();
        let result = &value["toolUseResult"];
        if block["is_error"] == true {
            // A failed call: the error message as a string.
            assert_eq!(result.as_str(), Some(output));
            strings += 1;
        } else {
            // A call that succeeded: an object that holds the output again.
            let map = result.as_object().expect("an object");
            assert!(
                map.values().any(|v| v.as_str() == Some(output)
                    || v.pointer("/content").and_then(|c| c.as_str()) == Some(output)),
                "the output is not repeated in {line:.200}"
            );
            objects += 1;
        }
    }
    assert!(
        objects > 0 && strings > 0,
        "{objects} objects, {strings} strings"
    );
}

#[test]
fn the_assembler_parses_a_generated_session_without_unparsed_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let session = generated(tmp.path(), 5 * MIB, SEED);
    let asm = Assembler::from_file(&session.path).unwrap();
    assert_eq!(asm.unparsed_count(), 0);
    assert!(asm.unknown_attachment_kinds().is_empty());
    let turns = asm.into_turns();

    let users = turns
        .iter()
        .filter(|t| matches!(***t, Turn::User(_)))
        .count();
    assert_eq!(users, session.prompts);
    let mut kinds = BTreeSet::new();
    let (mut calls, mut lazy, mut inline_results, mut injected) = (0, 0, 0, 0);
    let (mut readable, mut redacted, mut footers) = (0, 0, 0);
    for turn in &turns {
        match turn.as_ref() {
            Turn::User(user) => {
                injected += user
                    .items
                    .iter()
                    .filter(|i| matches!(i, Item::Injected { .. }))
                    .count();
            }
            Turn::Assistant(assistant) => {
                footers += usize::from(assistant.stats.is_some());
                for item in &assistant.items {
                    match item {
                        Item::ToolCall(call) => {
                            calls += 1;
                            kinds.insert(format!("{:?}", call.tool));
                            if let Some(result) = &call.result {
                                if result.output.is_lazy() {
                                    lazy += 1;
                                } else {
                                    inline_results += 1;
                                }
                            }
                        }
                        Item::Thinking {
                            thinking: Thinking::Text(_),
                            ..
                        } => readable += 1,
                        Item::Thinking {
                            thinking: Thinking::Redacted,
                            ..
                        } => redacted += 1,
                        Item::Injected { .. } => injected += 1,
                        _ => {}
                    }
                }
            }
            Turn::Compaction(_) => {}
        }
    }
    assert_eq!(calls, session.tool_calls);
    assert_eq!(lazy, session.large_outputs);
    assert!(inline_results > lazy);
    assert!(injected > 0 && readable > 0 && redacted > 0 && footers > 0);
    let expected: BTreeSet<String> = [
        ToolKind::Shell,
        ToolKind::Read,
        ToolKind::Edit,
        ToolKind::Grep,
    ]
    .iter()
    .map(|k| format!("{k:?}"))
    .collect();
    assert_eq!(kinds, expected);
}

#[test]
fn the_loader_and_the_assembler_agree_on_a_generated_session() {
    let tmp = tempfile::tempdir().unwrap();
    let fixture = generate_fixture(tmp.path(), 3 * MIB, SEED, LARGE_PERCENT).unwrap();
    let (turns, timings) = load_turns(&fixture);
    assert_eq!(timings.warnings, 0);
    assert_eq!(timings.turns, turns.len());
    assert!(timings.first_content <= timings.loaded);
    let expected = Assembler::from_file(&fixture.session.path)
        .unwrap()
        .into_turns();
    assert_eq!(turns, expected);
    assert!(first_content(&fixture.harness(), &fixture.id()) > std::time::Duration::ZERO);
}

#[test]
fn a_cached_fixture_is_reused_and_describes_the_same_session() {
    let tmp = tempfile::tempdir().unwrap();
    let first = cached_fixture(tmp.path(), MIB, SEED, LARGE_PERCENT).unwrap();
    let modified = fs::metadata(&first.session.path)
        .unwrap()
        .modified()
        .unwrap();
    let second = cached_fixture(tmp.path(), MIB, SEED, LARGE_PERCENT).unwrap();
    assert_eq!(
        fs::metadata(&second.session.path)
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    assert_eq!(first.session, second.session);
    // Another mix is another file.
    let dense = cached_fixture(tmp.path(), MIB, SEED, 0).unwrap();
    assert_ne!(dense.session.path, first.session.path);
    assert_eq!(dense.session.large_outputs, 0);
}

#[test]
fn small_sessions_are_indexed_into_ten_projects() {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("projects");
    write_small_sessions(&data_dir, 50, 3).unwrap();
    let harness = ClaudeHarness::with_data_dir(data_dir, Default::default());
    harness.refresh().unwrap();
    // The build reads every file once (the "cold" benchmark), and a second
    // build of unchanged files reads none (the "warm" one).
    assert_eq!(harness.scan_count(), 50);
    harness.refresh().unwrap();
    assert_eq!(harness.scan_count(), 50);
    let projects = harness.projects().unwrap();
    assert_eq!(projects.len(), 10);
    let sessions: usize = projects
        .iter()
        .map(|p| harness.sessions(&p.id).unwrap().len())
        .sum();
    assert_eq!(sessions, 50);
}

/// A loaded transcript of a generated session, drawn at `SIZE`.
const SIZE: Size = Size::new(200, 50);

/// Loads a generated session into a transcript the way the loader streams it,
/// but on this thread (the counters the tests read are per thread): the file
/// is fed to the assembler in chunks of 256 KiB, and every `Appended` and
/// `Updated` batch it returns is applied as it comes, ending with `Loaded`.
fn loaded_transcript(dir: &Path, bytes: u64) -> (Transcript, SyntheticSession) {
    let session = generated(dir, bytes, SEED);
    let data = fs::read(&session.path).unwrap();
    let mut asm = Assembler::new(crate::model::ByteSource::File(Arc::new(
        session.path.clone(),
    )));
    let mut transcript = new_transcript().loading(true).viewport(SIZE);
    let (mut appended, mut updated) = (0, 0);
    let mut apply = |transcript: &mut Transcript, updates: Vec<ModelUpdate>| {
        for update in updates {
            match &update {
                ModelUpdate::Appended(_) => appended += 1,
                ModelUpdate::Updated(_) => updated += 1,
                _ => {}
            }
            transcript.apply_update(&update);
        }
    };
    for chunk in data.chunks(256 * 1024) {
        let updates = asm.push_chunk(chunk);
        apply(&mut transcript, updates);
        let updates = asm.flush_held();
        apply(&mut transcript, updates);
    }
    let updates = asm.finish();
    apply(&mut transcript, updates);
    transcript.apply_update(&ModelUpdate::Loaded);
    // Many batches, and some of them updated turns that were already shown.
    assert!(
        appended > 5 && updated > 0,
        "{appended} appended, {updated} updated"
    );
    assert_eq!(transcript.turn_count(), asm.turns().len());
    (transcript, session)
}

#[test]
fn folded_large_outputs_are_not_materialised_by_the_load_or_by_rendering() {
    let tmp = tempfile::tempdir().unwrap();
    let (reads, calls) = (source_reads(), text_calls());
    let (mut transcript, session) = loaded_transcript(tmp.path(), 12 * MIB);
    assert!(session.large_outputs > 0);
    assert_eq!(text_calls(), calls, "the load materialised an output");
    assert_eq!(source_reads(), reads, "the load re-read the transcript");

    // Draw the whole session, folded, from the top to the end.
    let rows = usize::from(SIZE.height);
    let mut offset = 0;
    loop {
        transcript.scroll_to(offset);
        render(SIZE.width, SIZE.height, |frame| {
            transcript.view(frame, frame.area());
        });
        if transcript.offset() < offset {
            break;
        }
        offset += rows / 2;
        if offset > 100_000 {
            break;
        }
    }
    assert!(
        transcript.offset() > rows * 20,
        "only a short stretch was drawn"
    );
    assert_eq!(text_calls(), calls, "drawing materialised an output");
    assert_eq!(source_reads(), reads, "drawing re-read the transcript");
}

#[test]
fn the_frame_scenario_unfolds_a_large_output_and_that_does_materialise_it() {
    let tmp = tempfile::tempdir().unwrap();
    let (transcript, _) = loaded_transcript(tmp.path(), 12 * MIB);
    let (reads, calls) = (source_reads(), text_calls());
    let mut scenario = FrameScenario::new(transcript, SIZE);
    // The instrument works: unfolding the large output reads it.
    assert!(scenario.large_section.is_some());
    assert!(text_calls() > calls);
    assert!(source_reads() > reads);

    scenario.draw_once();
    let start = scenario.screen();
    assert!(start.lines().count() == usize::from(SIZE.height));
    assert!(start.lines().any(|l| !l.trim().is_empty()));
    for _ in 0..30 {
        scenario.scroll_and_draw();
    }
    assert_ne!(scenario.screen(), start, "scrolling did not move the view");
    scenario.rewind();
    scenario.draw_once();
    assert_eq!(
        scenario.screen(),
        start,
        "rewinding did not restore the view"
    );
}

#[test]
fn the_dense_mix_has_no_large_outputs() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join(format!("{SESSION_ID}.jsonl"));
    let session = generate_session_with(&path, 2 * MIB, SEED, 0).unwrap();
    assert_eq!(session.large_outputs, 0);
    let turns: Vec<Arc<Turn>> = Assembler::from_file(&path).unwrap().into_turns();
    assert!(!turns.is_empty());
}

#[test]
fn the_sidebar_key_scenario_moves_the_selection_and_the_transcript_one_block_at_a_time() {
    let tmp = tempfile::tempdir().unwrap();
    let (transcript, _) = loaded_transcript(tmp.path(), 4 * MIB);
    let rows = transcript
        .query(crate::components::TURN_ROWS_ATTR)
        .unwrap()
        .into_attr();
    let mut sidebar = TurnSidebar::new(builtin::default_theme());
    sidebar.attr(crate::components::TURN_ROWS_ATTR, rows);
    let entries = sidebar.rows().entry_count();
    assert!(entries > 40, "{entries} entries");
    let (reads, calls) = (source_reads(), text_calls());

    let mut scenario = SidebarKeyScenario::new(transcript, sidebar);
    let mut offsets = vec![scenario.transcript.offset()];
    for press in 1..=30 {
        let took = scenario.press_and_draw(true).expect("an entry below");
        assert!(took < Duration::from_secs(1), "{took:?}");
        assert_eq!(scenario.sidebar.selected_entry(), Some(press));
        offsets.push(scenario.transcript.offset());
        // The block is at the top of the screen.
        let screen = scenario.transcript_screen();
        let first = screen.lines().next().unwrap();
        assert!(first.starts_with('╭'), "{first}");
    }
    assert!(offsets.windows(2).all(|w| w[0] < w[1]), "{offsets:?}");

    // From the end, upwards; and the end of the list does nothing.
    scenario.to_end();
    assert_eq!(scenario.press_and_draw(true), None);
    assert!(scenario.press_and_draw(false).is_some());
    assert_eq!(scenario.sidebar.selected_entry(), Some(entries - 2));
    assert_eq!(text_calls(), calls, "the keys materialised an output");
    assert_eq!(source_reads(), reads, "the keys re-read the transcript");
}
