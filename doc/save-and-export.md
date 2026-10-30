# Saving and exporting

`w` / `Ctrl-s` save the selected section as displayed, `W` saves its full
content, `e` / `Ctrl-e` export the session as markdown as currently folded, and
`E` exports it in full (SPEC §6.4). Each asks for a path in the help line
(`Save to: ` / `Export to: `) with a default filled in, asks `Overwrite
<path>? (y/n)` when the file exists, and ends with a toast (`Saved to <path>`,
`Exported to <path>`, `Save failed: <error>`, `Export failed: <error>`).

- Default save name: `./<project>-<short-id>-<section-kind>.txt`, for example
  `./brio-85a86657-shell.txt`. The kind is `user`, `assistant` (also the
  `model → …` note), `thinking`, the tool's kind (`shell`, `read`, `write`,
  `edit`, `grep`, `glob`, `webfetch`, `websearch`, `subagent`, `tool`),
  `system`, `compaction` or `stats`. On the `… N more lines` row it is the tool
  call's.
- Default export name: `./<project>-<yyyymmdd-hhmm>-<short-id>.md`, with the
  session's start time in the display timezone (UTC with `utc = true`).
- `<project>` is the project's display name with every character outside
  `[A-Za-z0-9._-]` replaced by `-`; `<short-id>` is the first 8 characters of
  the session id.
- Relative paths are relative to the directory chronoscope was started in; `~`
  is the home directory. No directory is created: a missing parent is a
  failure.
- chronoscope never writes inside a harness's data directory (SPEC §9.5): a
  path there is refused with `Refusing to write into harness data`. Paths are
  compared after resolving `..` and symlinks.

## The markdown export

```
# <session title>

- Project: <name>
- Session: <id>
- Working directory: <cwd>
- Started: <timestamp>
- Harness: <name>

## User — <timestamp>
## Assistant (<model> @ <effort>) — <timestamp>
## Compaction — <timestamp>
```

Prompts and assistant text are written as their markdown source, thinking as a
block quote. In the folded export (`e`) a folded section is its summary as a
block quote (`> Find the BRIO on USB`; a folded prompt is the rows the screen
shows, as `y` copies them), an unfolded tool call is its
description in bold with each label (`Command:`, `Result:`) in bold above a
fenced code block with what is displayed (a truncated output ends in its
`_… N more lines_` row), and hidden system content is left out. In the full
export (`E`) nothing is folded, truncated or hidden, and every tool call is

````
<details><summary>description</summary>

```sh (or json)
input
```

```
output
```

</details>
````

Fences are one backtick longer than the longest run of backticks in their
content (at least three). A tool output that cannot be read back from the
transcript is written as `(output unavailable: <reason>)`.
