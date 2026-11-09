# Chronoscope — Coding

This file says how code is written in this project, which is Rust with tui-realm and Ratatui: how components are written, where files go and how they are named, the code that is shared, and what keeps the code portable. It is a living document: a cycle's docs task updates it when the cycle changes a rule. Rules are binding, and a rule that starts with `Prefer:` is a preference. `product §n` is a section of [product.md](product.md). How the parts fit together is in [architecture.md](architecture.md), and how they are tested in [testing.md](testing.md).

## Components

- **Every custom UI element is its own component.** The code is organised around components, not around long render functions or a central key handler.
- A new UI element gets a component of its own. It is never drawn by adding to another component's render function, or to `App::view`.
- A component implements tui-realm's `Component` trait (`view`, `query`/`attr`, `state`, `perform(Cmd) -> CmdResult`), plus `AppComponent` (`on(&Event) -> Option<Msg>`) if it handles events.
- An element that appears on several screens (e.g. the help line) is one component type, mounted with different props.
- A component draws only itself. Its `view` draws its own border and content, and leaves sub-elements to child types: a child component it owns, or a small `Widget`/`StatefulWidget` type.
- **No long render functions.** A render function (`view`, or a helper it calls) that draws more than one distinct element, or grows past about 50 lines, is split into smaller components or widgets.
- Rendering has no side effects.
- Components are `pub` types that can be built from plain data (props and the theme) and rendered into a `TestBackend` frame on their own, without an `Application` or a terminal ([testing.md](testing.md#what-is-tested)).
- Colours come from the theme's roles and tokens (product §7.1), passed in as a prop. Components never hard-code colours.
- The fixed modifiers of product §7.2 and §4.5 (bold, reverse video, italic, underline and strikethrough, which no theme token carries) come from one module, `src/theme/emphasis.rs`, which names each fixed style and joins it to the theme's colours where product §7.2 says so. No component, renderer or the markdown renderer writes a `Modifier` itself: outside `src/theme/`, production code never names `ratatui::style::Modifier` and never sets a modifier through a shortcut such as `.bold()`. Test code (`#[cfg(test)]` modules, files under `tests/`, fixtures) and `src/test_support.rs` may, to check a style.
- `tui-realm-stdlib` is not used. The look in product §3 and §7 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

## Files and modules

- Each component type has its own module under `src/components/`, named after the type in snake case (`TurnSidebar` is `src/components/turn_sidebar`). One type may be mounted under several `Id`s: `BackgroundFill` paints the `SidebarGap` and the `ScreenBackground`.
- Each harness adapter has its own module under `src/harness/`, named after the harness (`claude`, `opencode`), and implements the `Harness` trait ([architecture.md](architecture.md#data-flow)). Code that several adapters share sits beside them in `src/harness/`.

## Shared code

- Durations are formatted by `format_duration` in `src/duration.rs` (product §3.1), token counts by `format_tokens` in `src/tokens.rs` (product §4.2), and costs by `format_cost` in `src/cost.rs` (product §4.2, §3.3). The one other duration format is a tool call's under a minute (`340ms`, `1.2s`): `format_tool_duration` in `src/components/turn_block/renderers/tool/timing.rs` handles it and leaves a minute or more to `format_duration` (product §4.2). No component has a duration, token or cost formatter of its own beyond these.
- `src/components/text_fit.rs` fits a plain string into a number of display columns, for the components and the `App`: `truncate_with_ellipsis` cuts at the end with `…` after it, and `truncate_start` cuts at the start with `…` before it, so the end of a path stays visible.
- A component never depends on another component's internals. A helper that two components share gets a module of its own, as `PanelRow` does for the two session panels (`src/components/panel_row.rs`), and `text_fit`, `titled_block`, `list_cursor`, `turn_rows` and `double_click` do in `src/components/` ([architecture.md](architecture.md#components)). The details of a section as far as the model knows them, and the references the harness reads the rest from (`section_details`, `section_source`), are in `src/section_details/`, outside the details overlay: the `Transcript` calls them to build the overlay's request, and the overlay's `compose` uses the group names they define.
- Code that both harness adapters use sits in `src/harness/`, not in one adapter: the session title from a first prompt (`title_from_prompt` in `src/harness/shared.rs`, with `truncate_title`), the reading of a `notify` event by a session's watcher (`notice` in `src/harness/watch_event.rs`, after which each watcher matches only its own paths), and the resume command templates (`src/harness/resume.rs`).

## Portability

These rules keep macOS, the tier 2 platform (product §8.1), cheap to support.

- No direct OS APIs outside `src/platform.rs`; elsewhere only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera, ctrlc).
- `src/platform.rs` holds every use of `std::os::*` in production code, for what no cross-platform crate provides: `file_identity` (the device and inode of a file on Unix, to tell a replaced file apart) and `exec_or_run` (`exec` on Unix, for resume). Each function has a portable fallback for the other platforms (`created()`, and running the command and waiting for it), named in its doc comment. Test code may keep its own `#[cfg(unix)]` uses.
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
