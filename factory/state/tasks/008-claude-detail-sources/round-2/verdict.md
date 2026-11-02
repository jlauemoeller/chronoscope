VERDICT: REWORK

## Assessment

Round 2 fixed both round-1 must-fixes: assembly is linear again, and the notice, model-note and flush links have tests. Every acceptance criterion holds, and neither the reviewers nor I found wrong behaviour. What blocks is test coverage only: five required behaviours work today, but each can break with the whole suite still green. The fix is one set of tests, so this is a rework and not a human choice.

**Code review: FAIL**, four must-fixes and seven nits.
- Finding 1 (must-fix), the `stopped` flag has no test through the assembler: to be fixed. The planner notes bind "the message's latest line with a non-null `stop_reason`, or else its latest line".
- Finding 2 (must-fix), nothing tests that the footer reads its last `turn_duration` line: to be fixed. The planner notes bind this too.
- Finding 3 (must-fix), nothing tests the reference of entries held before a prompt: to be fixed. Without it the section silently loses its Environment group.
- Finding 4 (must-fix), nothing tests that a call sent again in its turn keeps its table entry: to be fixed. A regression silently drops every row read from the result's line, which the task requires.
- Finding 5 (nit), the timing test catches a re-added scan only some of the time: to be fixed, by widening it. Across the two reviews it missed the scan in 3 of 9 runs, so it is not a dependable guard as it stands.
- Finding 6 (nit), three other field locations have no test: to be fixed. The planner notes name the order of these locations, and one test covers all three.
- Finding 7 (nit), a call id from an earlier turn and `has_result`: the first is to be fixed, because it completes the pair with finding 4. The second is ignored; see Conflicts.
- Finding 8 (nit), a summary without a boundary: to be fixed, as feature finding 2.
- Finding 9 (nit), an empty `timestamp`: to be fixed, one line.
- Findings 10 and 11 (nits), a wrong figure in a comment and a duplicated test helper: to be fixed, because they are trivial and the round is happening anyway.

**Feature review: FAIL**, two must-fixes and five nits.
- Finding 1 (must-fix): the same gap as code finding 1. To be fixed.
- Finding 2 (must-fix), `held.add_to` and `own_sources` for a summary without a boundary: to be fixed. The first is the same gap as code finding 3.
- Finding 3 (nit), `unavailable_80x24_default` and the offset in the build notes: the change is accepted (see Conflicts), and round-2's notes stay as they are.
- Findings 4, 5 and 7 (nits): the same as code findings 5, 10, 7 and 9.
- Finding 6 (nit), the 0.66 s figure excludes the load path: the sentence in `docs/performance.md` is to be added, and the load path itself needs a home (below).

**UI review: PASS**, two nits.
- Finding 1 (nit), `{"decision": ""}` draws `decision=`: to be fixed. It is one line, and it closes a gap in round 2's own "an empty string is no value" rule.
- Finding 2 (nit), the offset in the build notes: ignored. The snapshot is right, and the correct figure is 7.
- In its states list, the UI review notes that a repeated call id's second result still goes to the first call: ignored. This routing is older than this task.

**Why the test gaps are not minor.** `factory.md` lists "a missing or ineffective test for required behaviour" as a must-fix, and I do not lower these:
- Each one guards a rule from the planner notes or §5.3.
- A regression would be silent: rows disappear from the overlay and nothing fails.
- Rarity is not a reason here. The code reviewer found no null line after a stopped one in 47 787 local lines, and I accept that. The 006 round-3 verdict still refused to call the same kind of gap minor while budget remained, and nothing here differs from that case.
- The budget is untouched, and the change is almost entirely tests, so the risk of another round is small.
- This is not the start of an open-ended chase. Both reviewers mutated the reference links, and of the gaps they list, only `has_result` stays unpinned after this note.

**Why this is not an A/B choice.** Each gap has one fix, a test, and none offers the human a functional choice.

**Independent checks**, on a fresh export of `HEAD` (79664a3) with its own target directory and `INSTA_UPDATE=no`:
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. `cargo test` gives 2840 passed, 0 failed, 2 ignored, and no `.snap.new` file is written.
- The merge base equals `main`.
- `git diff main...HEAD` adds 4 App-level snapshots and changes one. I read that one change: only the `model` row and its style line differ.
- I did not rerun the budgets on `HEAD`. The builder and two reviewers report them within budget (worst full parse 458.0 ms, peak heap 0.63x, slowest frame 5.2 ms), nobody disputes that, and the note changes nothing on the parse path.

With the rework note applied to a scratch copy:
- The gates pass, and `cargo test` gives 2847 passed, 0 failed, 2 ignored. No `.snap` file changes.
- Each temporary edit in the note's "Done when" fails exactly the test named there and no other, which also reproduces the reviewers' claim that the current suite misses these.
- With a scan re-added, the widened timing test failed 6 of 6 runs alone, at 95x to 148x against the limit of 48x, and failed again in a full suite. Without the scan it had no false failure in 18 runs, 12 of them full suites, some with other builds running alongside.

**Not blocking, but these need a home.** A search of `backlog.md` and the `task.md` of tasks 009 to 014 finds neither, so a backlog entry or a later task should pick them up:
- **The real load path on one long turn.** The feature reviewer measured `ClaudeHarness::load` on the 70 MB single turn at 3.4 s on this branch and 4.0 s on `main`, over the 2 s budget on both. Those are single runs on a loaded machine, and the cause was not profiled. This task did not cause it. I did not measure it myself.
- **The control-character filter in `display_lines`.** The 007 round-4 verdict recommended adding it in tasks 008 and 009, but it never reached a `task.md`. `display_lines` in `src/components/details_overlay/content.rs` still handles only tab and carriage return. This task now sends raw `cwd`, `gitBranch` and `permissionDecision` strings down that path. I left it out of the note because no reviewer raised it and the input is narrow: a control character between a base and U+FE0F, at least three times. I read the code and did not reproduce it this round.

## Conflicts

No two reviews make opposing demands. Four smaller points, of which only the second uses the priority order, to choose between two equivalent tests:

1. **The snapshot criterion and the UI section.** The criterion limits snapshot changes to App-level ones, but `unavailable_80x24_default` is a component snapshot, and it changed because the UI section says "one row with no label column". I accept the change. SPEC §7 allows changes to the rows a task intends to change, this one is intended, and all three reviews agree. The value moves from offset 13 to 7, not 9 as round 2's notes say, and the overlay stays 40 columns wide.
2. **Which sequence tests the stop rule.** The code and feature reviews propose different lines for the same gap. The note uses the code reviewer's sequence, by the priority order, and adds the feature reviewer's Environment assertion. The feature reviewer's three-line sequence is dropped because it adds no coverage.
3. **The timing test.** The code reviewer offers widening it or dropping it, and the feature reviewer asks to widen it. The note widens it, because it then catches the scan reliably in the ordinary `cargo test`, which `budgets` does not run.
4. **`has_result`.** The code reviewer says to remove it if no behaviour depends on it, and the feature reviewer says to test it if it is worth keeping. The note does neither. A behaviour does depend on it: an identical result repeated on a new line moves the reference. No spec rule pins that behaviour, though, so it needs no test, and removing it would change it for no gain.

## Rework budget

Earlier REWORK verdicts: 0 of 2. Round 1 has no `verdict.md`, because round 2 was the automatic fix round. This verdict is the first REWORK.

## Rework note

The behaviour is right. This round adds the tests that pin it, makes one small fix to an empty member, and corrects two comments and a doc sentence. Nothing else changes. All the test code below has been tried on the current branch.

**1. New tests in `tests/claude_details.rs`.** Add these before `an_empty_string_is_no_value`:

```rust
#[test]
fn a_line_after_the_stopped_line_leaves_the_message_group_on_the_stopped_line() {
    let mut later = first_line();
    later["uuid"] = json!("a3");
    later["timestamp"] = json!("2026-01-01T00:00:03Z");
    later["gitBranch"] = json!("later");
    later["message"]["content"] = json!([{"type": "text", "text": "More."}]);
    let t = transcript(&[prompt_line(), first_line(), call_line(), later]);
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert!(
        message.rows.contains(&"stop_reason=tool_use".to_owned()),
        "{message:?}"
    );
    assert!(
        message
            .rows
            .contains(&"timestamp=2026-01-01T00:00:02+00:00".to_owned()),
        "{message:?}"
    );
    let env = group(&groups, SourceGroupKind::Environment).unwrap();
    assert_eq!(env.rows, environment("main"));
}

#[test]
fn the_footer_reads_its_last_turn_duration_line() {
    let mut last = duration_line();
    last["uuid"] = json!("d2");
    last["timestamp"] = json!("2026-01-01T00:00:11Z");
    last["gitBranch"] = json!("last-footer");
    let t = transcript(&[prompt_line(), first_line(), duration_line(), last]);
    let refs = t.assistant().sources.get(&SourceKey::Footer).unwrap();
    let groups = read(&SectionSource {
        kind: SourceKind::Other,
        refs,
    });
    assert_eq!(
        group(&groups, SourceGroupKind::Environment).unwrap().rows,
        environment("last-footer")
    );
}

#[test]
fn a_hook_before_the_prompt_reads_the_environment_of_its_own_line() {
    let t = transcript(&[hook("h0", "before-prompt"), prompt_line(), first_line()]);
    assert!(matches!(&*t.turns[0], Turn::User(_)));
    let source = section(&t, 0, "injected:h0");
    assert_eq!(source.kind, SourceKind::Other);
    assert_eq!(environment_branch(&source), "gitBranch=before-prompt");
}

#[test]
fn a_summary_without_a_boundary_reads_the_environment_of_its_own_line() {
    let summary = json!({
        "type": "user", "uuid": "cs1", "isCompactSummary": true,
        "timestamp": "2026-01-01T00:05:01Z", "gitBranch": "summary-only",
        "message": {"role": "user", "content": "Summary"}
    });
    let t = transcript(&[prompt_line(), summary]);
    assert!(matches!(&*t.turns[1], Turn::Compaction(_)));
    let id = SectionId {
        turn: t.turns[1].id().clone(),
        key: SectionKey::Summary,
    };
    let source = section_source(&t.turns[1], &id).expect("a source for the summary");
    assert_eq!(source.kind, SourceKind::Other);
    assert_eq!(environment_branch(&source), "gitBranch=summary-only");
}

#[test]
fn a_call_sent_again_in_its_turn_keeps_the_rows_of_its_result() {
    let t = transcript(&[
        prompt_line(),
        first_line(),
        call_line(),
        call_line(),
        result_line(json!({"isAsync": true})),
    ]);
    let groups = read(&t.item("c1", SourceKind::ToolCall));
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert!(
        call.rows.contains(&"sandboxed=false".to_owned()),
        "{call:?}"
    );
    assert!(call.rows.contains(&"isAsync=true".to_owned()), "{call:?}");
    assert!(!call.unavailable);
}

#[test]
fn a_call_id_seen_in_an_earlier_turn_reads_its_own_line() {
    let mut second_prompt = prompt_line();
    second_prompt["uuid"] = json!("u2");
    second_prompt["timestamp"] = json!("2026-01-01T00:01:00Z");
    let mut again = call_line();
    again["uuid"] = json!("a9");
    again["timestamp"] = json!("2026-01-01T00:01:01Z");
    again["gitBranch"] = json!("later-turn");
    again["message"]["id"] = json!("m9");
    let t = transcript(&[
        prompt_line(),
        first_line(),
        call_line(),
        result_line(json!({})),
        second_prompt,
        again,
    ]);
    let turn = t.turns.len() - 1;
    let source = section(&t, turn, "c1");
    let groups = read(&source);
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert_eq!(call.lead, ["start=2026-01-01T00:01:01+00:00"]);
    assert_eq!(environment_branch(&source), "gitBranch=later-turn");
}

#[test]
fn the_other_field_locations_are_read_and_the_top_of_the_line_wins() {
    let mut call = call_line();
    call["diagnostics"] = json!({"cache_miss_reason": "top"});
    let mut result = result_line(json!({
        "permissionDecision": "from-result-object",
        "resolvedModel": "from-result-object"
    }));
    result.as_object_mut().unwrap().remove("permissionDecision");
    result["resolvedModel"] = json!("from-the-top");
    let t = transcript(&[prompt_line(), first_line(), call, result]);
    let groups = read(&t.item("m1:0", SourceKind::Message));
    let message = group(&groups, SourceGroupKind::Message).unwrap();
    assert!(
        message
            .rows
            .contains(&"diagnostics.cache_miss_reason=top".to_owned()),
        "{message:?}"
    );
    let groups = read(&t.item("c1", SourceKind::ToolCall));
    let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
    assert_eq!(
        call.rows,
        [
            "permissionDecision=from-result-object",
            "sandboxed=false",
            "resolvedModel=from-the-top",
        ]
    );
}
```

**2. Changes to existing tests in `tests/claude_details.rs`.**
- In `an_empty_string_is_no_value`, add `line["timestamp"] = json!("");` with the other fields, and add this as the last assertion:
  ```rust
  assert!(!message.rows.iter().any(|r| r.starts_with("timestamp")));
  ```
- In `assembling_a_long_turn_takes_time_in_proportion_to_its_items`, compare 3 000 and 48 000 items with a limit of 48x. The old limit caught a re-added scan only some of the time:
  ```rust
  // Looking each new item up in the references would make this quadratic:
  // sixteen times the items would take 256 times as long.
  let best = |n| (0..2).map(|_| time_long_turn(n)).min().unwrap();
  let (small, large) = (best(3_000), best(48_000));
  assert!(
      large < small * 48,
      "3000 items took {small:?}, 48000 took {large:?}"
  );
  ```
- In `a_compaction_and_a_system_section_read_their_first_line_for_the_environment`, delete the local `hook` closure, so the test uses the module-level function.

**3. An empty member is no value, in `src/harness/claude/details.rs`.** In `Member`'s `visit_str`:

```rust
fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
    Ok(Member((!value.is_empty()).then(|| value.to_owned())))
}
```

In the test `a_permission_decision_is_text_or_its_scalar_members`, add this assertion before the `for` loop:

```rust
assert_eq!(
    text(r#"{"decision":"","source":"config"}"#).as_deref(),
    Some("source=config")
);
```

Then add `r#"{"decision":""}"#` to the values in that loop, which must give `None`.

**4. Comments and docs.**
- `tests/budgets.rs`, the doc comment of `long_turn_failures`: change "~85 MB" to "~70 MB".
- `docs/performance.md`: replace the last two sentences of the paragraph on a single turn with very many items, from "The budget test checks it" on, so the paragraph ends:
  ```
  The budget test checks it with a turn of
  100 000 rounds of text, tool call and result (70 MB): full parse 0.66 s. That
  figure is `Assembler::from_memory` and leaves out the load path. Looking each
  new item up in the table first made this quadratic (tens of seconds), so
  `tests/claude_details.rs` also has a test that compares the time of 3 000 and
  48 000 items.
  ```

**Leave as is.**
- `has_result` in `src/model/sources.rs` and its use in `assemble.rs`. Do not remove it and do not add a test for it. Removing it would change which line a repeated result refers to, and no spec rule pins that behaviour.
- `unavailable_80x24_default`. Its change from round 2 is accepted, so do not revert it. The `model` value moved from offset 13 to 7, so write 7 if you mention it. Do not edit round 2's build notes.
- The load path on a single long turn. It is over budget on `main` too, and it is not part of this task.
- All other production code. The only production change in this round is the one in section 3.

**Done when.**
- Each of these temporary edits fails exactly the named test and no other. Make each edit, run `INSTA_UPDATE=no cargo test --no-fail-fast`, restore the code, and say so in the build notes:

  - `assemble.rs`: `.note_message_line(message_no, range, false)` fails `a_line_after_the_stopped_line_leaves_the_message_group_on_the_stopped_line`.
  - `sources.rs`, `set_footer`: `self.footer.is_none() && replace(&mut self.footer, range)` fails `the_footer_reads_its_last_turn_duration_line`.
  - `assemble.rs`, `start_user_turn`: removing the call `held.add_to(&mut sources, &self.source);` fails `a_hook_before_the_prompt_reads_the_environment_of_its_own_line`.
  - `assemble.rs`, `on_compact_summary`, the branch without a boundary: `sources: TurnSources::new(),` fails `a_summary_without_a_boundary_reads_the_environment_of_its_own_line`.
  - `assemble.rs`: `Some(known) if known.turn == b.idx && false => known.source,` fails `a_call_sent_again_in_its_turn_keeps_the_rows_of_its_result`.
  - `assemble.rs`: `Some(known) => known.source,` fails `a_call_id_seen_in_an_earlier_turn_reads_its_own_line`.
  - `details.rs`, `Rows::time`: removing `.filter(|v| !v.is_empty())` fails `an_empty_string_is_no_value`.
  - `details.rs`, `Member`'s `visit_str`: `Ok(Member(Some(value.to_owned())))` fails `a_permission_decision_is_text_or_its_scalar_members`.
  - `details.rs`, `message_group`: reading `cache_miss` from `message` only fails `the_other_field_locations_are_read_and_the_top_of_the_line_wins`.
  - `details.rs`, `tool_group`: removing `.or_else(|| from?.permission_decision.as_deref())` fails `the_other_field_locations_are_read_and_the_top_of_the_line_wins`.
  - `details.rs`, `tool_group`: reading `resolvedModel` from `toolUseResult` only fails `the_other_field_locations_are_read_and_the_top_of_the_line_wins`.
  - `sources.rs`, `add_message_item`: first calling `let _ = self.items.iter().rposition(|(item, _)| item == id);` fails `assembling_a_long_turn_takes_time_in_proportion_to_its_items`.

- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass with the code restored.
- `INSTA_UPDATE=no cargo test` passes with 2847 tests passing, 0 failing and 2 ignored (2840 plus the 7 new ones).
- `INSTA_UPDATE=no cargo test --release -- --ignored budgets --nocapture` stays within budget.
- No `.snap` file changes in this round, and no `.snap.new` file is left behind. A changed snapshot is a failure, so do not accept one.
