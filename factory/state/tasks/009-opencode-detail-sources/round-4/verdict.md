VERDICT: APPROVE

## Assessment

All three reviews pass, and every acceptance criterion is met with an effective test. Two nits remain, both minor enough to ignore. Round 4 did what the round-3 note asked: three new tests, one stronger WAL assertion and one reworded comment, with no production behaviour or `.snap` change.

**Code review: PASS**, one nit.
- Finding 1 (nit), the overlay opens the database on the UI thread with SQLite's 5 s busy timeout: ignored, and listed as a follow-up. The reviewer did not measure this, so I did, on a private export:

  | Journal mode | Writer holding `EXCLUSIVE` | Read time | Result |
  |---|---|---|---|
  | WAL | no | 0.2 ms | real values |
  | WAL | yes | 0.1 ms | real values |
  | Rollback journal | no | 0.1 ms | real values |
  | Rollback journal | yes | 5.0 s | three `unavailable` groups |

  - **It does not happen in normal use.** OpenCode's database runs in WAL mode (base spec §3.2), and there a held writer does not block the read.
  - **The worst case stays within §5.3.** The read ends in the `unavailable` fallback, with no panic and the overlay open. I did not test WAL recovery, which can also make a reader wait, but it is rare and ends the same way.
  - **The fix is outside this task.** The task's planner notes bind the reader to the existing `Db`, and a shorter timeout needs a new `Db` method.

**Feature review: PASS**, one nit.
- Finding 1 (nit), an OpenCode footer shows only the Turn group, though §5.3 gives Environment to "every section with a source": ignored, and listed as a spec clarification.
  - A footer sums several rows and has no row of its own, so there is no single `snapshot` to show.
  - The build notes made this choice in round 1, UI reviews have rendered it since then, and `the_footer_has_the_turn_group_and_nothing_read_from_a_row` pins it.
  - The reviewer asks for no code change and proposes no other behaviour, so this is not an A/B choice for the human.

**UI review: PASS**, no findings. Round-3 UI nit 1 is closed. Round-2 UI nits 2 and 3 stay declined, as that round's verdict ruled.

**Independent checks**, on a private export of `HEAD` (9656bff) with its own target directory and `INSTA_UPDATE=no`. The repo was not touched.
- **Gates:** `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass. `cargo test` gives 2905 passed, 0 failed, 2 ignored, and writes no `.snap.new` file.
- **Budgets:** `cargo test --release -- --ignored budgets --nocapture` passes.

  | Measure | Worst | Budget |
  |---|---|---|
  | First content | 2.4 ms | 300 ms |
  | Full parse, 100 MB fixtures | 420.5 ms | 2000 ms |
  | Full parse, one endless turn (70 MB) | 942.5 ms | 2000 ms |
  | Frame | 5.16 ms | 16 ms |
  | Peak heap | 0.63x the file | 3x |

- **The synchronous read:** these budgets do not measure OpenCode's overlay read, and no reviewer timed it. `section_details` on a tool call in a row with 10 MiB of `data` took 5.9 ms in a release build, so the read does not threaten the §5.2 requirement that the §9.2 budgets still hold.
- **Mutations:** the Code and Feature Reviewers each ran all eleven temporary edits from the note, with the results it names. I spot-checked three (`PartHead::time`, `Head::model` and the note's `let read = ...;`), and each failed only the test it names.
- **Snapshots:** `git diff main...HEAD` adds exactly the two required `opencode_details` snapshots and changes no other `.snap` file.
- **Assembler:** the diff adds no field to `AssistantData` or `UserData`. It shares `error_text` and records row ids.
- **Merge:** `main` has no commits past this branch's merge base.

Optional follow-ups, not conditions of this approval:
- Give the connection `section_details` opens a short busy timeout, so a locked database quickly gives `unavailable` (code finding 1).
- If the spec is revised, say in §5.3 that an OpenCode footer has no Environment group (feature finding 1).
- From the round-2 verdict: `SourceRow.replaces` holds a UI label in a model type, and `Db::open` can leave `-wal` and `-shm` files beside a WAL database that had none. The second comes from the shared `Db`, not this task.

## Conflicts

None.

## Rework budget

Earlier REWORK verdicts: 2 of 2 (rounds 2 and 3). The `approver-review.md` files in rounds 3 and 4 are those verdicts' notes and do not count. The budget played no part in this verdict, because no remaining finding needs a rework.
