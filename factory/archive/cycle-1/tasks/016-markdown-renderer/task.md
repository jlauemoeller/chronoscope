# Markdown and code rendering

## Goal
Build the text renderer the transcript uses for assistant text, user prompts and thinking (§5.5). It turns markdown into styled, wrapped Ratatui `Line`s for a given width, base style and theme:
- headings, emphasis, lists, inline code, block quotes and tables;
- fenced code blocks, syntax-highlighted with `syntect` and mapped to the theme's `syntax.*` tokens.

Prose is word-wrapped. Code is char-wrapped, and a char-wrap helper is exposed for commands and tool output. A small `Markdown` widget renders the result into a buffer, so the renderer can be snapshot-tested and reviewed on its own. Later tasks also use the code highlighter for `Write` tool content.

## Spec excerpts
From SPEC.md §5.5:

- Assistant text, user prompts and thinking are rendered as markdown (`pulldown-cmark`): headings, emphasis, lists, inline code, block quotes, tables, and fenced code blocks with syntax highlighting (`syntect`, mapped to theme colours).
- Prose is word-wrapped. Commands and tool output are char-wrapped (as in the sketch) so paths and long tokens are never hidden.
- Wrapping is recomputed when the terminal is resized.

From SPEC.md §8.1:

| Token | Default | Used for |
|---|---|---|
| `command` | `foreground` | Command / tool input text |
| `thinking` | `muted_foreground` | Thinking text |
| `syntax.*` | derived from `accent`, `primary`, `secondary`, `warning` | Code-block syntax highlighting |

From SPEC.md §1.3:

- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets. Rendering has no side effects.

- All colours and modifiers come from theme tokens (§8.1), passed in as a prop. Components never hard-code colours.

From SPEC.md §1.3 (Transcript parts):

The `Transcript` is **one** mounted component, because it is virtualised and can hold thousands of sections (§9.2). It does not mount one tui-realm component per section. It is still built from small parts rather than one large render function: a `TurnBlock` widget (rounded block, titles, selection marker), one renderer per section kind (§5.2), one body renderer per `ToolKind` (§5.3), the markdown renderer (§5.5) and the layout cache. Each is its own type with its own tests. Opening a subagent (§5.4) mounts a new `Transcript`. The parent's stays mounted but is not drawn, so `Backspace` returns to it unchanged.

From SPEC.md §9.2:

- Layout is computed lazily per block for the current width. Only blocks overlapping the viewport are rendered.

From SPEC.md §9.6:

- Adapter tests use fixture sessions (trimmed real sessions, scrubbed of secrets and personal data) for each harness, including subagents, compaction and live append scenarios.
- Component tests: every component (§1.3) is tested on its own. Its `view` is rendered into a Ratatui `TestBackend` frame and snapshotted with `insta`, and events passed to `on()` (or `Cmd`s to `perform`) are checked against the returned `Msg` / `CmdResult`.
- Update tests: `App::update(Msg)` is checked against the resulting domain state, mounted components and focus, without a terminal.
- Screen snapshot tests render `App::view` into a `TestBackend` with `insta`, for each built-in theme at a fixed size.
- Layout-cache property tests check that heights match the rendered output for random widths and fold states.
- Benchmarks (`criterion`) on a synthetic 100 MB session against the §9.2 budgets.

Planner notes (decisions not in the spec; binding for this task):
- **API.** `render_markdown(src: &str, width: u16, base: Style, theme: &Theme) -> Vec<Line<'static>>` is pure and deterministic. `char_wrap(text, width, style) -> Vec<Line>` wraps at exactly `width` display cells (`unicode-width`), preserving existing newlines and leading spaces. `highlight_code(code, lang_or_extension, theme) -> Vec<Line>` highlights without wrapping; callers wrap as needed. A `Markdown` widget (`pub`, implements `Widget`) renders `render_markdown` output into an area. `base` is the section's text style (e.g. `foreground` for assistant text, `thinking` for thinking). Every element style is derived from it by adding modifiers or token colours.
- **Element styles** (planner choice; colours only from theme tokens):
  - Headings: bold, with level 1 also underlined. No `#` characters are shown, and there is a blank line after them.
  - `*em*` is italic, `**strong**` bold, `~~strike~~` crossed out.
  - Inline code: the `command` token colour, with no backticks shown.
  - Links: link text underlined. The URL is not shown.
  - Bullet lists use `• ` and ordered lists `1. `. Nested lists are indented 2 cells per level, and continuation lines hang-indent under the item text.
  - Block quotes: each line is prefixed with `│ ` in `muted_foreground`, and the content keeps the base style.
  - Fenced code blocks: indented 2 cells, highlighted, and char-wrapped to the remaining width. An unknown language uses `syntax.plain`.
  - Tables: columns sized to content, separated by ` │ `, with a `─┼─` rule under the header row, in `muted_foreground` separators. A row that is too wide truncates the widest cells with `…` until it fits.
  - Horizontal rules: `─` across the full width in `muted_foreground`.
  - Blank lines separate blocks. There is no trailing blank line.
- **Syntax mapping.** Load syntect's default `SyntaxSet` once, lazily (e.g. `OnceLock`; this is not UI state). Map syntect scopes to the task-002 tokens instead of using a syntect colour theme:
  - `keyword`/`storage` → `syntax.keyword`
  - `entity.name.type`/`support.type` → `syntax.type`
  - `entity.name.function`/`support.function` → `syntax.function`
  - `string` → `syntax.string`
  - `constant.numeric`/`constant.language` → `syntax.number`
  - `comment` → `syntax.comment`
  - everything else → `syntax.plain`

  Building a syntect `Theme` programmatically from the tokens is one acceptable way to do this.
- Wrapping uses `textwrap` for prose. Measure in display cells (wide characters count 2). Never split a grapheme.

## Acceptance criteria
- [ ] `render_markdown` renders each element per the planner notes. Unit tests assert the lines and spans for: a heading of each level 1–3, emphasis/strong/strike, inline code, a link, nested bullet and ordered lists with wrapping, a block quote, a table, a horizontal rule, and a fenced `rust` block.
- [ ] Prose wraps at word boundaries within `width`. A single word longer than `width` is broken. Wide characters (e.g. `漢字`) are measured as 2 cells (tests).
- [ ] `char_wrap` breaks at exactly `width` cells regardless of spaces, and preserves leading indentation and blank lines (tests). It is the helper later tasks use for commands and tool output.
- [ ] Fenced code blocks are highlighted: in a `rust` block, `fn` uses `syntax.keyword`, a string literal `syntax.string`, a comment `syntax.comment` and a number `syntax.number` (styled test). An unknown language falls back to `syntax.plain`.
- [ ] Changing the theme changes the colours, with no hard-coded colours anywhere in the module: a test renders with two themes and compares styles.
- [ ] Rendering is deterministic and side-effect free, and the `SyntaxSet` is loaded at most once (tests).
- [ ] The `Markdown` widget snapshots in the UI section exist.
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
A new standalone widget, `Markdown`, used later inside transcript blocks. A sample document contains:
- an H1 and an H2;
- a paragraph with `*em*`, `**strong**`, `` `code` `` and a link;
- a nested bullet list and an ordered list;
- a block quote;
- a 3-column table;
- a fenced `rust` code block;
- a horizontal rule.

Rendering follows the element styles in the planner notes: no `#` or backticks shown, `• ` bullets, `│ ` quote prefix, ` │ ` table separators with a `─┼─` header rule, and code indented 2 cells.

Snapshots (component):
1. Sample document at width 80 (text + styled).
2. Sample document at width 40, where prose wraps, the code block char-wraps and the table truncates cells with `…` (text).
3. Code block only, with `rust` highlighting, at width 60 (styled), showing `syntax.keyword`, `syntax.string`, `syntax.comment` and `syntax.number`.

Planner note — snapshot conventions (binding for every snapshot in this task):
- Render through the `test_support` helpers from task 001 (Ratatui `TestBackend`), and accept the
  snapshots with `insta`. Take a text snapshot for the layout, and also a styled snapshot (one that
  shows fg/bg/modifiers per cell run) wherever this task specifies colours or modifiers.
- Use the `default` built-in theme unless a state says otherwise.
- Snapshots must be deterministic on every machine and in CI. Use fixed fixture data and timestamps,
  a fixed timezone (configure the timestamp formatter with `utc = true` or an injected fixed
  offset), and an injected "now" for anything time-dependent.

## Depends on
- 002-theme-model
