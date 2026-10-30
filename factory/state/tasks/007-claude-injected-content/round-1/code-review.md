VERDICT: FAIL

## Summary
The classification (allowlist, noise, metadata, unparsed), compaction, version check, unknown-kind logging and the new `inject.rs` module are clean and well tested. Field choices match real `~/.claude/projects` data. Real data shows that `queued_command` puts `origin` under `attachment` and that a human queued message gets no duplicate `user` entry. One collapsing bug remains: `flush_current` merges consecutive injected entries into a trailing `Injected` item for Assistant turns but not for User turns. On the live-append path (and in a few static sequences), consecutive entries in a User turn with no reply yet become two sections instead of one.

## Test gate
- cargo fmt --check: pass
- cargo clippy --all-targets -- -D warnings: pass
- cargo test: pass

## Findings
1. [must-fix] `src/harness/claude/assemble.rs`, `flush_current`, the `else if` branch for the last turn being a `Turn::User`. Here held entries are always appended with `turn.items.push(item)`. The Assistant branch above it instead merges into a trailing `Item::Injected`. As a result, consecutive allowlisted entries split across an end-of-batch flush do not collapse (SPEC §5.6, AC 2, and the planner rule that `flush_held` must follow the same rules for live appends).
   - I reproduced this in a scratch copy:
     - Input: prompt `P`, attachment `A`, `flush_held()`, attachment `B`, reply.
     - Live: the User turn ends up with `[Injected[A], Injected[B]]`.
     - Same bytes assembled whole: one `Injected[A, B]`.
     - So the live view differs from reopening the file. `@`-mentioned `file` attachments and `UserPromptSubmit` hook messages are written right after the prompt, so a live batch boundary landing among them is realistic.
   - The same asymmetry shows without `flush_held` for `P, A, system turn_duration, B, reply`: the User branch gives two items, while the equivalent Assistant-turn case gives one.
   - Change:
     - In the User branch, if `turn.items.last_mut()` is `Some(Item::Injected { entries, .. })`, extend `entries` with the new entries (keeping the existing item's id) instead of pushing a second item, as the Assistant branch does.
     - Add a test mirroring `flushing_held_entries_mid_stream_keeps_a_partial_line_for_later` for a User turn with no reply yet: `P`, `A`, `flush_held()`, `B`, reply. Assert the User turn has exactly one `Injected` item with two entries, and that the replayed updates equal `asm.turns()`.
2. [nit] `src/harness/claude/assemble.rs`, `process`, `"attachment"` arm: the `attachment.starts_with("deferred_tools_")` check lives in the assembler, while the rest of the classification is in `inject.rs`. It also makes the two explicit `deferred_tools_*` entries in `NOISE_ATTACHMENTS` redundant. Move the prefix check into `is_known_noise_attachment` in `src/harness/claude/inject.rs`.
3. [nit] `src/harness/claude/assemble.rs`, `process`, `"attachment"` arm: an attachment without a `type` (`unwrap_or_default()` gives `""`) is counted as an unknown kind and logged as `unknown attachment kind "" (1×)`. Skip empty kinds in the counter, or record them under a readable label such as `<missing>`.
4. [nit] `src/harness/claude/assemble.rs`, `end_of_input` / `into_turns`: held entries are silently dropped when the input ends after a Compaction turn, or before any turn. This is consistent with the planner rule ("go into the next one"). A transcript that was just compacted ends exactly like that, though: the real pattern is boundary, summary, then `compact_file_reference`/`file` attachments. Add a `log::debug!` with the number of entries dropped, so this shows up in the log.
5. [nit] `src/harness/claude/assemble.rs`, `flush_current`, Assistant branch: entries separated by a `tool_result` or `turn_duration` are merged into the previous `Injected` item. This goes beyond the planner's "only noise between them", although it is visually adjacent and documented in the build notes. Consider correcting the comment "Only turn stats or tool results lie between" to say this merge is deliberate.
6. [nit] `src/harness/claude/assemble.rs`, `check_blocks`: if the same unknown block is re-sent on a repeated assistant line (SPEC §5.1 step 3), it is counted once per line, which inflates the note. Low impact; consider de-duplicating by `(message.id, block index)` if it shows up in practice.
7. [nit] `src/harness/claude/assemble.rs`, `end_of_input`: in live mode, each unknown kind is logged once with its count at the first flush, so later occurrences are never reflected in the log. This is acceptable under "once per kind per session". The accessor `unknown_attachment_kinds()` stays accurate.
8. [nit] `src/harness/claude/assemble.rs`, `on_attachment(&mut self, kind: &str, entry: Entry, ...)`: `entry` is taken by value but only borrowed. Take `&Entry`.
9. [nit] Real local data contains unknown attachment kinds the build notes do not list (`structured_output`, `read_truncation_notice`). They are worth adding to the noise list or allowlist in a follow-up so they stop showing up in the log.
