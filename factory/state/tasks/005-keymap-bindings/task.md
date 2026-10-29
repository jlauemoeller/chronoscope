# Keymap: actions, default bindings, remapping and chords

## Goal
Implement the shared keymap that every component uses to turn key events into actions (§1.4, §8.5). It covers:
- the action set per context, with the default bindings of §6;
- parsing of the key syntax;
- user overrides from `[keys.<context>]` in `config.toml`, with warnings for bad entries;
- two-key chords (user config only);
- resolution of tui-realm key events to actions;
- display labels and help metadata (group + description) for every action, so the help line and help overlay can be generated from the active keymap.

## Spec excerpts
From SPEC.md §1.4:

- **Keys go through the keymap.** A component's `on()` resolves the key with the shared keymap (§8.5), for its own context, to an action. The action becomes a `Cmd` (handled internally with `perform`) or a `Msg`. Components never match literal `KeyEvent`s, except that `Prompt` takes printable characters as text. This keeps remapping and the generated help overlay (§4.3) correct.
- **Global keys** (quit, `?`, `t`, `Ctrl-o`) are handled by `GlobalEvents`, which subscribes to them (`Sub` / `SubClause`) using the active keymap, instead of every component checking for them. Subscribers get events as well as the focused component, so a `SubClause` keeps them from firing while `Prompt`, `HelpOverlay` or `ThemeSelector` has focus. Those components handle their own keys (input mode, §6; `Esc` / `?` closes help). `Ctrl-c` always quits. In the selector, `t`, `?` and `q` still work (§6.6).

From SPEC.md §6:

All keybindings are remappable in config (§8.5). The defaults avoid modifier+Enter combinations, which most terminals cannot distinguish from Enter.

**Input mode:** while a help-line prompt is active (search, file path, y/n confirmation), the `Prompt` component has focus (§1.4) and every printable key goes into the prompt. Only `Enter`, `Esc`, `Backspace`, the arrow keys and `Ctrl-c` keep their meaning there. This prevents conflicts such as `j` / `t` / `1`–`4` while typing a search.

From SPEC.md §6.1:

| Key | Action |
|---|---|
| `Enter` / `Space` | Toggle fold of current section |
| `←` / `h` | Fold current section |
| `→` / `l` | Unfold current section |
| `-` | Fold all |
| `+` / `=` | Unfold all |

Fold levels (`1`/`2`/`3`) and Vim-style `z` aliases (`za`, `zc`, `zo`, `zM`, `zR`) were considered and rejected. No default binding is a chord.

From SPEC.md §6.3:

| Key | Action |
|---|---|
| `↓` / `j`, `↑` / `k` | Select next / previous section |
| `]` / `[` | Jump to next / previous turn block (User / Assistant / Compaction) |
| `Ctrl-d` / `Ctrl-u` | Scroll half a page down / up |
| `PgDn` / `PgUp` | Scroll a full page down / up |
| `g` / `Home`, `G` / `End` | Jump to start / end (`G` resumes following, §7.2) |

- `Ctrl-b` / `Ctrl-f` are deliberately not bound: `Ctrl-b` is tmux's default prefix.

From SPEC.md §6.4:

| Key | Action |
|---|---|
| `y` | Copy the selected section **as displayed** (folded: summary line; unfolded: everything shown) as plain text |
| `Y` | Copy the selected section's **full content** (description + full input/command + full result), regardless of fold state or truncation |
| `w` / `Ctrl-s` | Save the selected section as displayed to a file |
| `W` | Save the selected section's full content to a file |
| `e` / `Ctrl-e` | Export the session to markdown **as currently folded** (folded sections become their summary lines) |
| `E` | Export the **full** session to markdown (tool calls as `<details>` blocks with fenced input/output) |

From SPEC.md §6.5:

| Key | Action |
|---|---|
| `Ctrl-o` | Open the session selector (plain `o` is not bound) |
| `a` | Open the selected subagent's transcript (§5.4) |
| `Backspace` | Back to the parent session (after `a`) |
| `R` | Resume the session in its harness (§6.7) |
| `s` | Toggle visibility of system / injected content |
| `t` | Open the theme selector (§8.4) |
| `?` | Help overlay |
| `q`, `Ctrl-q`, `Ctrl-w`, `Ctrl-c` | Quit |

`Ctrl-c` always quits; it is never used for copy.

From SPEC.md §6.6:

| Key | Action |
|---|---|
| `1`–`4` | Switch harness tab (the number shown next to its name) |
| `Tab` / `Shift-Tab` | Move focus between Projects and Sessions |
| `←` / `h`, `→` / `l` | Focus Projects / Sessions (no wrap-around: `←` in Projects and `→` in Sessions do nothing) |
| `↑↓` / `j` `k` | Move in the focused list |
| `/` | Fuzzy search sessions (§4.1) |
| `Enter` | Open the selected session; in the Projects pane, move focus to Sessions |
| `Esc` | Clear search; otherwise close the selector (if a session is already open) or quit (at startup) |
| `R` | Resume the selected session in its harness (§6.7) |
| `t`, `?`, `q` / `Ctrl-q` / `Ctrl-c` | Theme selector, help overlay, quit |

- Harness numbers are stable: `1` Claude, `2` OpenCode, `3` Codex, `4` Pi.
- Harness tabs are switched only by number or by clicking a tab (§6.9). `←` / `→` move between the panes, never between tabs; there is no previous/next-harness action.

From SPEC.md §8.5:

```toml
[keys.browser]                 # action = key or [keys]; replaces the default for that action
fold_toggle = ["enter", "space"]
copy = "y"
copy_full = "Y"

[keys.selector]
search = "/"
```

Key syntax: `a`, `A`, `ctrl-d`, `shift-tab`, `enter`, `esc`, `pgdn`, `f5`, and two-key chords as space-separated keys (`"g g"`; only in user config, since no default binding is a chord). The help overlay is generated from the active keymap, so it always reflects the user's remappings.

From SPEC.md §9.4:

- No feature depends on the kitty keyboard protocol or on modifier+Enter. Every default binding works with legacy key encoding.

From SPEC.md §4.3:

- **Help overlay** (`?`): grouped keybindings as in ui-sketch.md, generated from the active keymap. `Esc` / `?` closes it.

Planner notes (decisions not in the spec; binding for this task and for every later task that handles keys):
- **Contexts** and their config tables: `global` (`[keys.global]`), `browser` (`[keys.browser]`), `selector` (`[keys.selector]`), `overlay` (`[keys.overlay]`, used by the help overlay and the theme selector). The `Prompt` input mode is not remappable.
- **Actions.** Name → default keys, help group, and help description:

  | Context | Action | Default keys | Group | Description |
  |---|---|---|---|---|
  | global | `quit` | `q`, `ctrl-q`, `ctrl-w` | Application | Quit |
  | global | `help` | `?` | Application | Show keybindings |
  | global | `theme` | `t` | Application | Choose theme |
  | global | `open_selector` | `ctrl-o` | Sessions | Open another session |
  | browser | `prev` | `up`, `k` | Navigation | Previous section |
  | browser | `next` | `down`, `j` | Navigation | Next section |
  | browser | `prev_turn` | `[` | Navigation | Previous turn |
  | browser | `next_turn` | `]` | Navigation | Next turn |
  | browser | `half_page_up` | `ctrl-u` | Navigation | Scroll half a page up |
  | browser | `half_page_down` | `ctrl-d` | Navigation | Scroll half a page down |
  | browser | `page_up` | `pgup` | Navigation | Scroll a page up |
  | browser | `page_down` | `pgdn` | Navigation | Scroll a page down |
  | browser | `top` | `g`, `home` | Navigation | Jump to start |
  | browser | `bottom` | `G`, `end` | Navigation | Jump to end and follow |
  | browser | `fold_toggle` | `enter`, `space` | Sections | Fold or unfold current section |
  | browser | `fold` | `left`, `h` | Sections | Fold current section |
  | browser | `unfold` | `right`, `l` | Sections | Unfold current section |
  | browser | `fold_all` | `-` | Sections | Fold all sections |
  | browser | `unfold_all` | `+`, `=` | Sections | Unfold all sections |
  | browser | `toggle_system` | `s` | Sections | Show or hide system content |
  | browser | `copy` | `y` | Sections | Copy section as displayed |
  | browser | `copy_full` | `Y` | Sections | Copy full section |
  | browser | `save` | `w`, `ctrl-s` | Sections | Save section as displayed |
  | browser | `save_full` | `W` | Sections | Save full section |
  | browser | `search` | `/` | Search | Search transcript |
  | browser | `search_next` | `n` | Search | Next match |
  | browser | `search_prev` | `N` | Search | Previous match |
  | browser | `clear_search` | `esc` | Search | Clear search |
  | browser | `open_subagent` | `a` | Sessions | Open subagent transcript |
  | browser | `back` | `backspace` | Sessions | Back to parent session |
  | browser | `resume` | `R` | Sessions | Resume in harness |
  | browser | `export` | `e`, `ctrl-e` | Application | Export session as folded |
  | browser | `export_full` | `E` | Application | Export full session |
  | selector | `harness_1` … `harness_4` | `1` … `4` | Session selector | Switch to Claude / OpenCode / Codex / Pi |
  | selector | `next_pane` | `tab` | Session selector | Next pane |
  | selector | `prev_pane` | `shift-tab` | Session selector | Previous pane |
  | selector | `focus_projects` | `left`, `h` | Session selector | Focus Projects |
  | selector | `focus_sessions` | `right`, `l` | Session selector | Focus Sessions |
  | selector | `up` | `up`, `k` | Session selector | Move up |
  | selector | `down` | `down`, `j` | Session selector | Move down |
  | selector | `search` | `/` | Session selector | Search sessions |
  | selector | `open` | `enter` | Session selector | Open session |
  | selector | `cancel` | `esc` | Session selector | Clear search, close or quit |
  | selector | `resume` | `R` | Session selector | Resume in harness |
  | overlay | `up` | `up`, `k` | Overlays | Move up |
  | overlay | `down` | `down`, `j` | Overlays | Move down |
  | overlay | `select` | `enter` | Overlays | Select |
  | overlay | `close` | `esc` | Overlays | Close |

  `Ctrl-c` is not in the keymap. It always quits, and is handled literally by the global event handler and by `Prompt`.
- **Key syntax.**
  - Optional modifier prefixes `ctrl-`, `alt-`, `shift-` (case-insensitive, combinable).
  - Then either a named key or a single character. Named keys (case-insensitive): `enter`, `esc`, `tab`, `backspace`, `space`, `up`, `down`, `left`, `right`, `home`, `end`, `pgup`, `pgdn`, `delete`, `insert`, `f1`–`f12`.
  - Single characters are case-sensitive (`y` ≠ `Y`). `shift-<letter>` normalises to the uppercase letter.
  - A chord is two such keys separated by one space (`"g g"`).
  - Anything else is a parse error naming the string.
- **Matching.**
  - The keymap resolves `tuirealm::event::KeyEvent`.
  - For character keys the SHIFT modifier is ignored, because terminals report `G` and `?` with or without SHIFT.
  - `BackTab` matches `shift-tab`, and `Char(' ')` matches `space`.
  - `ctrl-d` matches `Char('d')` with CONTROL.
- **Overrides.** A config entry `action = "key"` or `["k1", "k2"]` replaces that action's default keys entirely. Each of the following produces a warning and is skipped, keeping the default:
  - an unknown context or action name;
  - an unparsable key;
  - a chord in the `global` or `overlay` context (chords are only supported in `browser` and `selector`);
  - a key in another context that collides with a `global` key (both would fire).

  A user key that collides with a default key of another action in the same context removes it from that action, with a warning. The user binding wins.
- **Chords.** A small `KeyResolver` holds the pending chord prefix. Each component owns one as presentation state. `feed(key)` returns `Action(a)`, `Pending` or `Unbound`. With a prefix pending:
  - if the next key completes a chord, it returns that action;
  - otherwise the prefix is dropped and the key is resolved afresh.

  There is no timeout. A key that is a chord prefix in a context is only a prefix there: a single-key binding of the same key in that context is dropped with a warning at load.
- **Labels.**
  - Characters display as themselves (`y`, `Y`, `/`, `?`).
  - Named keys display as `ENTER`, `ESC`, `TAB`, `Shift-TAB`, `SPACE`, `Backspace`, `Up`, `Down`, `Left`, `Right`, `Home`, `End`, `PgUp`, `PgDn`, `Delete`, `Insert` and `F5`.
  - Modifiers display as `Ctrl-x` / `Alt-x`. Chords display as `g g`.
  - `Keymap::label(context, action)` returns the first key's label (for help-line hints). `Keymap::labels(context, action)` returns all keys joined with ` / ` (for the help overlay), e.g. `w / Ctrl-s`.
- **Help metadata.** `Keymap::help_entries(context)` returns `(group, keys label, description)` rows in table order, reflecting the active (remapped) bindings.

## Acceptance criteria
- [ ] Every action in the planner-notes table exists with exactly the listed default keys, group and description. A table-driven test asserts the full default keymap.
- [ ] No default binding is a chord and none uses modifier+Enter (a test iterates all defaults).
- [ ] The key syntax parser accepts every form in the planner notes and §8.5 (`a`, `A`, `ctrl-d`, `shift-tab`, `enter`, `esc`, `pgdn`, `f5`, `"g g"`) and rejects `ctrl-`, `foo`, `f13` and `"a b c"` with an error naming the string (tested).
- [ ] Resolution maps tui-realm `KeyEvent`s to actions per context:
  - `Char('G')` with and without SHIFT resolves to `bottom`;
  - `BackTab` resolves to `prev_pane`;
  - `Char(' ')` resolves to `fold_toggle`;
  - `Char('d')`+CONTROL resolves to `half_page_down`;
  - `Char('c')`+CONTROL resolves to nothing in every context.

  Each case is tested.
- [ ] `[keys.browser] copy = "c"` makes `c` resolve to `copy` and `y` resolve to nothing. `[keys.browser] fold_toggle = ["enter", "space"]` keeps the defaults (tested).
- [ ] Each of these produces a warning and keeps the default: an unknown action name, an unknown context, an unparsable key, `[keys.global] quit = "g g"`, and `[keys.browser] copy = "t"` (collides with global `theme`). Each case is tested.
- [ ] A same-context collision (`[keys.browser] copy = "n"`) moves `n` to `copy`, removes it from `search_next`, and warns (tested).
- [ ] With `[keys.browser] top = "g g"`, feeding `g` then `g` to a `KeyResolver` yields `Pending` then `Action(top)`. Feeding `g` then `j` yields `Pending` then `Action(next)` (tested).
- [ ] `label`/`labels` produce the planner-notes formats: `labels(browser, save)` is `w / Ctrl-s`, `labels(browser, prev)` is `Up / k`, and `label(selector, next_pane)` is `TAB`. They reflect remappings (tested).
- [ ] `help_entries(browser)` returns rows in table order with groups Navigation, Sections, Search, Sessions, Application, and shows remapped keys (tested).
- [ ] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test` pass.

## UI
UI: none

## Depends on
- 003-config-file
