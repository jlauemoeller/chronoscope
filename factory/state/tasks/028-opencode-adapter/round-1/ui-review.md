VERDICT: FAIL

## Summary
All three snapshots the `UI` section asks for exist and match it. The tab row reads `[1] Claude  [2] OpenCode`. The selected tab is `primary` (229,229,229) with a `━` underline exactly as wide as its label. The other tab, and an unavailable OpenCode tab, are `muted_foreground` (161,161,161) with no underline, and an unavailable OpenCode cannot be selected. The Assistant title shows `acme/coder-1 @ high` only for the non-`default` variant. I also rendered the views end to end with the real `OpenCodeHarness`, `SessionIndex` and `App::view`. That showed one must-fix: OpenCode's Read tool body numbers the file content wrongly, so the OpenCode Read body is not the same as Claude's and breaks SPEC §5.3. `INSTA_UPDATE=no cargo test` passes (all binaries, 0 failed).

## States checked
- Selector, OpenCode selected, projects and sessions listed @ 120x40 — snapshot `selector_with_opencode_selected_120x40_text` / `_styled` — ok. Tabs are two spaces apart and the underline covers columns 17..29 under `[2] OpenCode`. `Alpha App` and `beta-tool` (the basename of a project without `name`) are listed, sessions are newest first, and the stats read `2 turns · 30m` / `1 turn · 10m`. Colours are `primary` and `muted_foreground` as required.
- Selector, Claude selected, OpenCode unavailable (muted) @ 80x24 — snapshot `an_unavailable_opencode_is_muted_and_cannot_be_selected_80x24_text`, plus the extra `an_unavailable_opencode_tab_is_muted_foreground_80x24_styled` — ok. `[2] OpenCode` is fg 161 with no underline, `[1] Claude` is fg 229 with `━`, and pressing `2` keeps Claude selected.
- Browser, OpenCode session with the shell tool unfolded and a non-default variant @ 120x40 — snapshot `an_opencode_session_in_the_browser_with_a_shell_tool_unfolded_120x40_text` — ok. It shows `Assistant ── acme/coder-1 @ high`, a `Command:`/`Result:` body like Claude's, and the footer `17s · 2 messages · 3.3k tokens · $0.03`. I checked that footer against the fixture: duration 17 s, tokens 1000+100+2000+200 = 3.3k, cost 0.01+0.02 = $0.03.
- Selector, OpenCode selected @ 80x24, 60x20, 40x15 (too small) — rendered in scratch crate, real index over the fixture DB — ok. The tabs are never truncated, and 40x15 shows "Terminal too small".
- Selector, beta-tool project @ 80x24 — scratch — ok. `● Investigate why t…` is shown for `ses_long` because `time_idle IS NULL` (SPEC §7.2). `Broken rows` and `(untitled)` also appear.
- Selector, OpenCode db_path missing, `2` pressed @ 80x24 (styled) — scratch, real `available()` path — ok. The tab is muted and Claude stays selected.
- Selector, Claude data dir missing, OpenCode available @ 80x24 (styled) — scratch — ok. OpenCode is preselected in `primary` with `━`, Claude is muted, and `1` does not switch.
- Selector, long project name (worktree basename of 76 characters) @ 80x24 and 120x40 — scratch — ok. The name is truncated with `…`.
- Browser, `ses_main` folded @ 120x40, 80x24 and 60x20, and bottom of the session @ 120x60 (text and styled) — scratch — ok. The second Assistant turn has no `@ default`. `model → acme/coder-2` appears, the trailing `synthetic` sits in the current turn, and the footer reads `39s · 2 messages · 880 tokens · $0.01`. See nit 1 for narrow titles.
- Browser, every section of `ses_main` unfolded @ 120x200 (text and styled) — scratch — issue with Read (finding 1). Everything else is ok:
  - The system entry shows its description, then its text.
  - Thinking and Text render as for Claude.
  - The failed `shell` (`cargo test`) and the failed `Write` (`Permission denied: edit`) show their results in `destructive` (255,100,103) when selected, like Claude's `failed_shell_call` snapshot.
  - The subagent sections show `Prompt:`/`Result:` and `a to open transcript`.
- Browser, edge session @ 120x40, 80x30, 60x30 and unfolded @ 100x80 — scratch, with extra rows: a long description, long command, `execute` (Other), `glob`, an `edit` with `filePath`/`oldString`/`newString`, and a long model id and variant — ok. The edit diff renders from the camelCase input, `Other` shows pretty JSON, and the long command wraps. See nit 1 for the title.
- Browser, `ses_untitled`, `ses_long` (live, user row only), `ses_bad` (2 unparsed rows), `ses_blank` (no rows) @ 100x30 — scratch — ok:
  - The header shows `2 entries could not be parsed` for `ses_bad` and `● live` for `ses_long`.
  - The multi-line first prompt is folded to one line.
  - See nit 3 for `ses_blank`.
- Test gate — `INSTA_UPDATE=no cargo test` — ok (0 failed in every binary).

## Findings
1. [must-fix] Browser, an OpenCode session with a `read` tool unfolded (`ses_main`, `call-read-1`), every size (seen at 120x60) — the Read body's `Content:` gutter numbers OpenCode's raw result text. This contradicts SPEC §5.3 ("file content with line numbers") and the UI requirement that OpenCode uses "the same … tool bodies as Claude". OpenCode's read result (fixture: `"Read file /home/user/alpha/src/main.rs\n1: fn main() {}\n"`) starts with a `Read file <path>` header line and prefixes each line with `N: `. `ReadBody::numbered` only recognises Claude's `N→` and `N<TAB>` prefixes, so the header line becomes gutter line `1` and the real line 1 becomes `2  1: fn main() {}`. Every OpenCode Read is drawn with a bogus first line and line numbers off by one, plus a duplicated `N: ` prefix in the text. Change: draw the OpenCode Read body as
   ```
   Content:

      1  fn main() {}
   ```
   with no `Read file …` line and the `N: ` numbers moved into the gutter. You can do this in either place:
   - in the adapter (`opencode/assemble.rs`): strip the leading `Read file <path>` line from a `read` tool's result, and any OpenCode framing such as a trailing end-of-file note if the real format has one (check the real DB read-only);
   - in `renderers/tool/read.rs`: also accept a `N: ` prefix in `parse_numbered`, and skip a leading `Read file ` header line before the first-line format check.

   Add a test that pins this, for example a turn_block or app snapshot of the unfolded OpenCode Read section, or an assembler test asserting the result the Read body receives. The required snapshot 3 only unfolds the shell tool, which is why this was not caught. See `ui/browser-read-unfolded-120x60.txt`.
2. [nit] Browser, Assistant title on narrow terminals (60x20 with `ses_main`; 80x30 and 60x30 with a long model id) — `acme/coder-1 @ high` becomes `acme/coder-1 @ hi…` at 60 columns. A long model id (`anthropic/claude-opus-5-5-extended-context-preview-20261001`) truncates before `@ extra-high-reasoning`, so the variant is truncated to `@ extra-high-reas…` at 120 columns, and disappears entirely at 100 and 80 columns. This is the shared title truncation from the turn-block task, and the `UI` section does not pin narrow sizes. A possible change: when the title must be truncated, truncate the model id and keep `@ <variant>` whole. See `ui/browser-narrow-title.txt`.
3. [nit] Browser, an OpenCode session without rows (`ses_blank`) @ 100x30 — the empty state reads "Session file is empty", although an OpenCode session is a set of database rows, not a file. This is the fixed SPEC §1.3/§4.2 text, so it should only change with a spec change, for example a harness-neutral "Session is empty". See `ui/browser-empty-opencode-session-100x30.txt`.
4. [nit] Browser, OpenCode `shell` calls without a `description` input (`cargo test`, `sleep 60`) — the folded header is just `▶ shell`, which follows the shared rule (`<tool name>`), as Claude shows `Bash`. Nothing to change for this task. Showing the first line of `command` would tell such calls apart, but that is a cross-harness spec change.
