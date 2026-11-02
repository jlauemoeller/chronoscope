VERDICT: PASS

## Summary
Everything the round-4 rework note asked for is done: the five tests, the added values and `parse_line` match it, no `.snap` file changed in this round, and all 14 temporary edits fail the test the note names. The test gaps of round 3 are closed, 15 further edits of my own were all caught, and I found no breach of a requirement. Four nits remain. The first is a real but unobserved robustness gap, in which a string `toolUseResult` with a lone surrogate escape shows `unavailable`.

Method: the edits ran on a private export of HEAD (3375e03) with its own target directory, using `cargo test --lib --test claude_details --no-fail-fast`. So they show that the named test fails and that no other test in those two targets fails. They do not cover the other test binaries. The gate below ran on the branch itself.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass (2852 passed, 0 failed, 2 ignored; no `.snap.new` file, no tracked file changed)
- cargo test --release -- --ignored budgets --nocapture (also required by §7, run on the export while the machine was busy): pass. Mixed: first content worst 2.3 ms, full parse worst 141.0 ms, peak heap 0.22x, worst frame 4.85 ms, details of the largest tool result (2 MB) 1.5 ms. Dense: first content worst 1.8 ms, full parse worst 451.4 ms, peak heap 0.63x, worst frame 1.58 ms. One endless turn: 939.5 ms.

## Findings
1. [nit] `src/harness/claude/details.rs:278-279`, with `ObjectVisitor` at `:456` — a few values that the assembler accepts make the details reader treat a line as unparseable. `pick_object` calls `deserialize_any`, which decodes a string or number before any visitor sees it, and `serde_json` rejects a lone `\ud83d` and a number out of range.

   Reproduced through the adapter on a tool call with `"toolUseResult":"cut \ud83d here"`:
   - The Tool call group comes back as `lead: [start], rows: [], unavailable: true`. `permissionDecision`, `sandboxed` and `isAsync` at the top of that line are lost.
   - `Assembler` reads the same bytes with `unparsed_count() == 0`, and the call has its result reference.
   - The same text inside a `toolUseResult` object is skipped and causes no trouble.

   Why this is a nit and not a must-fix:
   - The outcome is the fallback §5.3 specifies for an unparseable line: one muted row, no panic, the overlay stays open.
   - The untagged head §3 prescribes fails on the same values, as the doc comment on `wire::ToolUseResult` says.
   - Task 004 made this input a must-fix because it was a regression in assembly: the line was counted as unparsed and the call lost its result. Here nothing regresses, and the transcript is unaffected.
   - A scan of my local Claude transcripts (912 files, 143 218 lines) found 253 lines with a string `toolUseResult` and no line with a lone surrogate anywhere.

   Two points against that rating:
   - A JavaScript writer can produce a lone surrogate by cutting a string inside a surrogate pair, so the input is possible.
   - Read literally, the planner rule that a field of an unexpected type "never fails the line" covers `"sandboxed":1e400` and `"toolUseResult":1e999`, though `JSON.stringify` cannot write either value.

   Change, if wanted: read this one field as raw text first, which skips the value without decoding it.
   - In `Line`, use `#[serde(default, rename = "toolUseResult", deserialize_with = "pick_raw")]`.
   - Add this function after `pick_object`:
   ```rust
   /// [`pick_object`] for a value that may be large or damaged: it is taken as
   /// raw text, which skips it without decoding it, so a lone surrogate escape
   /// in it cannot fail the line, and only then read as `T`.
   fn pick_raw<'de, D: Deserializer<'de>, T: serde::de::DeserializeOwned>(
       d: D,
   ) -> Result<Option<T>, D::Error> {
       let raw = <&'de serde_json::value::RawValue>::deserialize(d)?;
       let mut json = serde_json::Deserializer::from_str(raw.get());
       Ok(pick_object::<_, T>(&mut json).ok().flatten())
   }
   ```
   - Add this test to `tests/claude_details.rs`, before `a_very_large_tool_use_result_is_skipped_and_its_fields_are_read`:
   ```rust
   #[test]
   fn a_tool_use_result_string_cut_in_a_surrogate_pair_keeps_the_rows_of_its_line() {
       let mut result = result_line(json!("cut XXXXXX here"));
       result["isAsync"] = json!(false);
       let t = transcript(&[prompt_line(), first_line(), call_line(), result]);
       // `json!` cannot write a lone surrogate. Same length, so the references hold.
       let text = fs::read_to_string(&t.path)
           .unwrap()
           .replace("XXXXXX", "\\ud83d");
       fs::write(&t.path, text).unwrap();
       let groups = read(&t.item("c1", SourceKind::ToolCall));
       let call = group(&groups, SourceGroupKind::ToolCall).unwrap();
       assert!(!call.unavailable, "{call:?}");
       assert_eq!(
           call.rows,
           [
               "permissionDecision=decision=accept, reasonType=classifier, source=config",
               "sandboxed=false",
               "isAsync=false",
           ]
       );
   }
   ```
   Tried on the export, exactly as above: the test fails on HEAD and passes with the change. `cargo fmt --check` and clippy pass, and the full suite gives 2853 passed (2852 plus this test), 0 failed, 2 ignored. A probe of `parse_line` with the change shows that a bare `1e999` in `toolUseResult` also parses, but a string with invalid UTF-8 in that field still fails the line, which a JavaScript writer does not produce. A top-level `"sandboxed":1e400` is another field and is not touched by this change. The budgets were not rerun with the change. It adds a second scan of the value when the overlay opens, and assembly is untouched.

2. [nit] `src/harness/claude/details.rs:11-17` — the module comment is not quite accurate. It calls `toolUseResult`'s reader an "object-or-ignored head", but a string there is decoded and not ignored. It also lists a duplicate key as the one way a line can still fail, and finding 1's values fail it too. Change: state those cases in that comment, so the limit is documented in one place.

3. [nit] `src/components/mod.rs:33-34` — `compose` is a very general name for a crate-level `components` export. Change: re-export it as `compose_details`, or call it through `details_overlay::compose`.

4. [nit] `src/components/transcript/details.rs:10-12` — the doc comment was edited without rewrapping, which leaves one short line (`reads the rest from. With the `… N more lines` row`). Change: rewrap the comment.
