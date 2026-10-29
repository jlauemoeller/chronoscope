# Chronoscope

Chronoscope is a TUI app for browsing agent (harness) sessions. It allows the user to select and browse a session, displaying the agent-user conversation. For agent turns, the user can see the description, command, and ouput of tool use. The app allows elements in the conversation to be folded to reduce clutter and allow the user to focus on the highlevel parts of the conversation before diving into specific parts. 

Refer to [ui-sketch](ui-sketch.md) for an outline of the UI and [theme-selector](theme-selector.md) for the theme selector.

Highlights:

- Initially supports Claude and OpenCode sessions with Codex and Pi planned. The UI is harness agnostic and abstracts away the fact that each harness stores session information in a proprietary format.

- Listens for changes in open session (either by looking for file events or by polling a data store) and updates display accordingly.

- Themable

- Keyboard first but does also support mouse

- Configuration stored in TOML file

