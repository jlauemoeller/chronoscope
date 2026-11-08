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
- All colours and modifiers come from theme tokens (product §7.1), passed in as a prop. Components never hard-code colours.
- `tui-realm-stdlib` is not used. The look in product §3 and §7 is specific enough that wrapping stdlib components would mean overriding their rendering. Prompt text editing uses `tui-input` inside the `Prompt` component.

## Files and modules

- Each component type has its own module under `src/components/`, named after the type in snake case (`TurnSidebar` is `src/components/turn_sidebar`). One type may be mounted under several `Id`s: `BackgroundFill` paints the `SidebarGap`.
- Each harness adapter has its own module under `src/harness/`, named after the harness (`claude`, `opencode`), and implements the `Harness` trait ([architecture.md](architecture.md#data-flow)). Code that several adapters share sits beside them in `src/harness/`.

## Shared code

- Durations are formatted by `format_duration` in `src/duration.rs` (product §3.1), and token counts by `format_tokens` in `src/tokens.rs` (product §4.2). The one other duration format is a tool call's under a minute (`340ms`, `1.2s`): `format_tool_duration` in `src/components/turn_block/renderers/tool/timing.rs` handles it and leaves a minute or more to `format_duration` (product §4.2). No component has a duration or token formatter of its own beyond these.
- A component never depends on another component's internals. A helper that two components share gets a module of its own, as `PanelRow` does for the two session panels (`src/components/panel_row.rs`).

## Portability

These rules keep macOS, the tier 2 platform (product §8.1), cheap to support.

- No direct OS APIs; only cross-platform crates (crossterm, notify, arboard, rusqlite bundled, etcetera).
- All filesystem paths are `PathBuf`; no hard-coded `/` separators or `~` expansion by string concatenation.
