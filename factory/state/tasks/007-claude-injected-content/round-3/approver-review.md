## Rework note
The behaviour is correct. This round adds a missing regression test and fixes two small doc and log issues. Do not change the placement logic.

1. **Required: test for the `user_flushed` reset.**
   - Add this test to `tests/claude_injected.rs` next to `entries_before_and_after_a_prompt_stay_in_separate_items`. The existing test only covers the first User turn, where `user_flushed` is still `false` from `new()`, so it cannot catch a missing reset.

     ```rust
     #[test]
     fn entries_before_and_after_a_later_prompt_stay_in_separate_items() {
         let text = lines(&[
             prompt("u1", "one"),
             hook("a1", "A"),
             reply("r1", "msg_1", "hi"),
             hook("x2", "X"),
             prompt("u2", "two"),
             hook("b2", "B"),
             reply("r2", "msg_2", "ho"),
         ]);
         let turns = assemble(&text).into_turns();
         let items = &user(&turns[2]).items;
         assert_eq!(item_names(items), ["injected", "injected"]);
         assert_eq!(injected(&items[0])[0].text, "X");
         assert_eq!(injected(&items[1])[0].text, "B");
     }
     ```

   - **Done when:**
     - The test passes at HEAD.
     - It fails when you temporarily delete `self.user_flushed = false;` from `start_user_turn` in `src/harness/claude/assemble.rs`. Run that mutation check, then restore the line.
   - In the build notes, report only mutation checks you actually ran, and what each one showed.
   - Optional: a live variant that calls `flush_held()` after `hook("b2", "B")` and asserts the same result.
2. **Required: correct the `into_turns` doc comment** in `src/harness/claude/assemble.rs` (around line 147). It currently says held entries "are placed in the current one". Make it say:
   - held entries go into the current User or Assistant turn if there is one;
   - with no turn yet, or a Compaction turn last, they are dropped and logged at debug.
3. **Required: fix the grammar of the `end_of_input` debug message.** Write "1 injected entry" and "N injected entries" according to the count.
   - Optional: avoid repeating this message on every `flush_held()` while the same entries stay held, for example by logging only when the held count changes. Do not restructure `flush_held`, `finish` or `end_of_input` for this.
4. **Out of scope:** do not change the re-sent unknown-block counting (feature nit 4) or round-1 nits 6, 7 and 9.
5. **Gate:** `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` all pass.
