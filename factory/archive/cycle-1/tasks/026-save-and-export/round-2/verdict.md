VERDICT: APPROVE

## Assessment

**Code review: PASS.** The round-1 must-fix is closed. A dangling symlink into harness data is now refused before any overwrite question is asked. `a_dangling_symlink_into_harness_data_is_refused_without_asking` covers this and would fail on the round-1 code. I re-ran the save tests with `INSTA_UPDATE=no` (65 passed) and the tree stayed clean.
- #1 [nit] The success toast cuts long paths at the wrong width at 60–63 columns. **Ignored.** It is cosmetic and only shows at the narrowest supported widths with long paths. The toast still uses the planner texts and the task-012 style, and the AC snapshot sizes render correctly. It could be a follow-up: cut the path against the real `toast_rect` width.
- #2 [nit, security] `resolved_in` fails open when it runs out of hops. **Ignored, with reasoning.** I read `resolved_in` (`src/app/save.rs`). It spends a hop only when it follows a link in the part of the path that does not exist yet. `canonicalize` resolves links in the existing prefix without spending hops. So the kernel always counts at least as many symlink traversals as the walk does.
  - A chain of exactly 40 dangling links is fully resolved, and its target is checked.
  - A chain the walk gives up on has more than 40 links, so `std::fs::write` fails with `ELOOP` (the limit is 40 on Linux and 32 on macOS). Nothing can reach harness data.
  - Failing closed when hops run out would be good defensive hardening for a follow-up. It is not blocking.
- #3 [nit] Carried-over items: the unused `full` field, the missing scroll marker in the shared `Prompt`, and the cost of building `E` on the UI thread. **Ignored.** The planner fixed the `full` field. The scroll marker behaviour predates this task. The `E` cost is now documented in `doc/save-and-export.md`.

**Feature review: PASS.** Every acceptance criterion is traced to an effective test.
- #1 [nit] `doc/save-and-export.md` does not mention that folded summaries escape `&` and `<`. **Ignored.** This is a documentation gap only, and the module doc in `export.rs` already describes the escaping.
- #2 [nit, security] The same hop-limit issue as code #2. **Ignored** for the same reason.
- #3 [nit] `a_loop_of_symlinks_ends` only checks that the call returns. **Ignored.** This is about test strength. The behaviour is correct.
- #4 [nit, security] A hard link in the working directory to an existing harness file is not detected. **Ignored as out of scope.** Exploiting it needs a planted hard link and a `y` on the overwrite prompt. The planner fixed both the write method (`std::fs::write`) and the refusal rule (paths inside the harness data directories). Detecting this by inode (`nlink > 1` or `(dev, ino)` against the watch paths) is a reasonable follow-up task.

**UI review: PASS.** All four required states have snapshots, and they match the UI section. The extra styled and `Saved to …` snapshots also match.
- #1 [nit] Toast cut at 60–63 columns. This is the same as code #1. **Ignored**, as above.
- #2 [nit] The path prompt draws no `…` marker when its input is scrolled. **Ignored.** This is existing shared `Prompt` behaviour and an optional follow-up.

## Conflicts
None. Code and Feature both raise the hop-limit nit, and Code and UI both raise the toast-width nit. In both cases the reviewers agree.

## Rework budget
Earlier REWORK verdicts: 0 of 2.
