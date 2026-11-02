//! What the help overlay lists (SPEC §4.3): the keybindings grouped by
//! purpose, generated from the active keymap, and how they are laid out in
//! rows.

use ratatui::layout::Size;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::keymap::{Action, Binding, Context, Keymap};
use crate::model::HarnessId;
use crate::theme::{Role, Theme, Token};

/// What follows the keys of `quit`: `Ctrl-c` always quits.
const CTRL_C_LABEL: &str = "Ctrl-c";
/// The row after the last group when mouse capture is on.
pub const SHIFT_NOTE: &str = "Hold Shift to select text with the mouse";
/// The overlay is at least this wide.
pub const MIN_WIDTH: u16 = 40;
/// The column of a group heading, counted from the overlay's left edge (the
/// border is column 0).
const HEADING_COLUMN: usize = 3;
/// The column of the keys of an entry.
const KEYS_COLUMN: usize = 5;
/// The columns between the end of the widest keys label and the description.
const KEYS_GAP: usize = 6;
/// The columns the overlay is wider than its widest entry: the key column's
/// offset, two columns of padding and the right border, less the left border
/// that is part of the offset.
const WIDTH_CHROME: usize = 8;
/// The rows of the border.
const BORDER_ROWS: usize = 2;

/// Where the help overlay was opened from; it decides what is listed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpContext {
    /// The session browser: the browser actions and the global ones.
    Browser,
    /// The session selector, also when it is over the browser.
    Selector,
}

/// One row of a group: the keys of an action and what it does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpRow {
    /// All keys of the action, joined with ` / `.
    pub keys: String,
    pub description: &'static str,
}

/// A group of rows under a heading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpGroup {
    pub title: &'static str,
    pub rows: Vec<HelpRow>,
}

/// The groups of the browser, in the order they are listed.
const BROWSER_GROUPS: [&str; 5] = [
    "Navigation",
    "Sections",
    "Search",
    "Sessions",
    "Application",
];
/// The groups of the selector.
const SELECTOR_GROUPS: [&str; 2] = ["Session selector", "Application"];
/// The global actions that are listed, in the order they come after the
/// context's own rows of the same group.
const GLOBAL_ACTIONS: [Action; 4] = [
    Action::OpenSelector,
    Action::Theme,
    Action::Help,
    Action::Quit,
];

/// The rows of the help overlay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HelpContent {
    groups: Vec<HelpGroup>,
    mouse_note: bool,
}

impl HelpContent {
    /// The content for `context`, from `keymap`.
    ///
    /// `registered` are the harnesses of the selector: only their
    /// `harness_N` rows are listed. `mouse` is whether mouse capture is on:
    /// then a note about `Shift` follows the last group. Actions without
    /// keys are left out.
    pub fn new(
        keymap: &Keymap,
        context: HelpContext,
        registered: &[HarnessId],
        mouse: bool,
    ) -> Self {
        let (own, titles): (Context, &[&str]) = match context {
            HelpContext::Browser => (Context::Browser, &BROWSER_GROUPS),
            HelpContext::Selector => (Context::Selector, &SELECTOR_GROUPS),
        };
        let global = |group: &str| {
            GLOBAL_ACTIONS
                .iter()
                .filter_map(|&action| keymap.binding(Context::Global, action))
                .filter(|b| b.group == group)
                .filter(move |b| {
                    context == HelpContext::Browser || b.action != Action::OpenSelector
                })
                .map(|b| row_of(keymap, Context::Global, b))
                .collect::<Vec<_>>()
        };
        let groups = titles
            .iter()
            .map(|&title| {
                let mut rows: Vec<HelpRow> = keymap
                    .bindings(own)
                    .iter()
                    .filter(|b| b.group == title && is_listed(b.action, registered))
                    .map(|b| row_of(keymap, own, b))
                    .collect();
                rows.extend(global(title));
                rows.retain(|row| !row.keys.is_empty());
                HelpGroup { title, rows }
            })
            .filter(|group| !group.rows.is_empty())
            .collect();
        Self {
            groups,
            mouse_note: mouse,
        }
    }

    /// The groups, in the order they are listed.
    pub fn groups(&self) -> &[HelpGroup] {
        &self.groups
    }

    /// Whether the note about `Shift` follows the groups.
    pub fn has_mouse_note(&self) -> bool {
        self.mouse_note
    }

    /// The number of rows the content takes: the blank first row, per group a
    /// heading, a blank row, its entries and a blank row, and the note.
    pub fn row_count(&self) -> usize {
        let groups: usize = self.groups.iter().map(|g| g.rows.len() + 3).sum();
        1 + groups + usize::from(self.mouse_note)
    }

    /// The width of the keys column: the widest keys label and the gap.
    fn keys_width(&self) -> usize {
        let widest = self
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .map(|row| row.keys.width())
            .max()
            .unwrap_or(0);
        widest + KEYS_GAP
    }

    /// The size the overlay wants, borders included: as wide as the widest
    /// entry plus [`WIDTH_CHROME`] (at least [`MIN_WIDTH`]), and as high as
    /// the content plus the border.
    pub fn desired_size(&self) -> Size {
        let description = self
            .groups
            .iter()
            .flat_map(|g| &g.rows)
            .map(|row| row.description.width())
            .max()
            .unwrap_or(0);
        let width = (self.keys_width() + description + WIDTH_CHROME).max(usize::from(MIN_WIDTH));
        Size::new(
            u16::try_from(width).unwrap_or(u16::MAX),
            u16::try_from(self.row_count() + BORDER_ROWS).unwrap_or(u16::MAX),
        )
    }

    /// The rows, top to bottom, styled with `theme`.
    pub fn lines(&self, theme: &Theme) -> Vec<Line<'static>> {
        let heading = Style::new()
            .fg(theme.role(Role::Foreground))
            .add_modifier(Modifier::BOLD);
        let key_style = theme.style(Token::HelpKey);
        let text_style = theme.style(Token::HelpText);
        let keys_width = self.keys_width();
        let mut lines = vec![Line::default()];
        for group in &self.groups {
            lines.push(indented(
                HEADING_COLUMN,
                vec![Span::styled(group.title, heading)],
            ));
            lines.push(Line::default());
            for row in &group.rows {
                let padding = " ".repeat(keys_width.saturating_sub(row.keys.width()));
                lines.push(indented(
                    KEYS_COLUMN,
                    vec![
                        Span::styled(row.keys.clone(), key_style),
                        Span::raw(padding),
                        Span::styled(row.description, text_style),
                    ],
                ));
            }
            lines.push(Line::default());
        }
        if self.mouse_note {
            lines.push(indented(
                HEADING_COLUMN,
                vec![Span::styled(SHIFT_NOTE, text_style)],
            ));
        }
        lines
    }
}

/// A line whose first span starts at overlay column `column`; the interior
/// starts at column 1.
fn indented(column: usize, mut spans: Vec<Span<'static>>) -> Line<'static> {
    spans.insert(0, Span::raw(" ".repeat(column - 1)));
    Line::from(spans)
}

/// Whether the overlay lists `action`: every action, except the switch to a
/// harness that is not registered.
fn is_listed(action: Action, registered: &[HarnessId]) -> bool {
    let number = match action {
        Action::Harness1 => 1,
        Action::Harness2 => 2,
        Action::Harness3 => 3,
        Action::Harness4 => 4,
        _ => return true,
    };
    HarnessId::from_number(number).is_some_and(|h| registered.contains(&h))
}

/// The row of `binding`, an action of `context`. The keys of `quit` are
/// followed by `Ctrl-c`, which always quits and is not part of the keymap. An
/// unbound action has no keys (the row is then dropped, except for `quit`).
fn row_of(keymap: &Keymap, context: Context, binding: &Binding) -> HelpRow {
    let labels = keymap.labels(context, binding.action);
    let keys = match (binding.action, labels) {
        (Action::Quit, Some(keys)) => format!("{keys} / {CTRL_C_LABEL}"),
        (Action::Quit, None) => CTRL_C_LABEL.to_owned(),
        (_, keys) => keys.unwrap_or_default(),
    };
    HelpRow {
        keys,
        description: binding.description,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RawKeys;

    const ALL: [HarnessId; 4] = [
        HarnessId::Claude,
        HarnessId::OpenCode,
        HarnessId::Codex,
        HarnessId::Pi,
    ];

    fn browser(keymap: &Keymap, mouse: bool) -> HelpContent {
        HelpContent::new(keymap, HelpContext::Browser, &[HarnessId::Claude], mouse)
    }

    fn selector(registered: &[HarnessId]) -> HelpContent {
        HelpContent::new(&Keymap::default(), HelpContext::Selector, registered, true)
    }

    fn keymap_with(context: &str, action: &str, keys: &[&str]) -> Keymap {
        let mut raw = RawKeys::new();
        raw.entry(context.to_owned()).or_default().insert(
            action.to_owned(),
            keys.iter().map(|k| (*k).to_owned()).collect(),
        );
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        keymap
    }

    fn rows(content: &HelpContent, title: &str) -> Vec<(String, &'static str)> {
        content
            .groups()
            .iter()
            .find(|g| g.title == title)
            .unwrap_or_else(|| panic!("no group {title}"))
            .rows
            .iter()
            .map(|r| (r.keys.clone(), r.description))
            .collect()
    }

    fn titles(content: &HelpContent) -> Vec<&'static str> {
        content.groups().iter().map(|g| g.title).collect()
    }

    #[test]
    fn the_browser_lists_its_groups_in_order() {
        let content = browser(&Keymap::default(), true);
        assert_eq!(
            titles(&content),
            [
                "Navigation",
                "Sections",
                "Search",
                "Sessions",
                "Application"
            ]
        );
    }

    #[test]
    fn the_browser_lists_every_browser_and_relevant_global_action_with_its_keys() {
        let keymap = Keymap::default();
        let content = browser(&keymap, true);
        let labels = |context, action| keymap.labels(context, action).unwrap();
        // Every browser action, in table order, in its group.
        for group in [
            "Navigation",
            "Sections",
            "Search",
            "Sessions",
            "Application",
        ] {
            let expected: Vec<(String, &str)> = keymap
                .bindings(Context::Browser)
                .iter()
                .filter(|b| b.group == group)
                .map(|b| (labels(Context::Browser, b.action), b.description))
                .collect();
            let shown = rows(&content, group);
            assert_eq!(&shown[..expected.len()], &expected[..], "{group}");
        }
        let total: usize = content.groups().iter().map(|g| g.rows.len()).sum();
        assert_eq!(total, keymap.bindings(Context::Browser).len() + 4);
        // The global ones come after the group's own rows.
        let sessions = rows(&content, "Sessions");
        assert_eq!(
            sessions.last().unwrap(),
            &("Ctrl-o".to_owned(), "Open another session")
        );
        let application = rows(&content, "Application");
        let tail: Vec<&str> = application[application.len() - 3..]
            .iter()
            .map(|r| r.1)
            .collect();
        assert_eq!(tail, ["Choose theme", "Show keybindings", "Quit"]);
    }

    #[test]
    fn the_rows_follow_the_planner_notes() {
        let content = browser(&Keymap::default(), true);
        let navigation = rows(&content, "Navigation");
        assert_eq!(navigation[0], ("Up / k".to_owned(), "Previous section"));
        assert_eq!(navigation[1], ("Down / j".to_owned(), "Next section"));
        assert_eq!(navigation[2], ("[".to_owned(), "Previous turn"));
        assert_eq!(navigation[3], ("]".to_owned(), "Next turn"));
        let sections = rows(&content, "Sections");
        assert!(sections.contains(&("+ / =".to_owned(), "Unfold all sections")));
        assert!(sections.contains(&("-".to_owned(), "Fold all sections")));
        assert!(sections.contains(&("y".to_owned(), "Copy section as displayed")));
        assert!(
            !sections.iter().any(|r| r.0.contains("Ctrl-c")),
            "Ctrl-c is never copy"
        );
    }

    #[test]
    fn a_remapped_copy_key_shows_on_the_copy_row() {
        let keymap = keymap_with("browser", "copy", &["c"]);
        let content = browser(&keymap, true);
        let sections = rows(&content, "Sections");
        assert!(sections.contains(&("c".to_owned(), "Copy section as displayed")));
        assert!(!sections.contains(&("y".to_owned(), "Copy section as displayed")));
    }

    #[test]
    fn a_remapped_key_with_several_keys_shows_them_all() {
        let keymap = keymap_with("browser", "copy", &["c", "ctrl-y"]);
        let sections = rows(&browser(&keymap, true), "Sections");
        assert!(sections.contains(&("c / Ctrl-y".to_owned(), "Copy section as displayed")));
    }

    #[test]
    fn an_unbound_action_has_no_row() {
        let keymap = keymap_with("browser", "copy", &[]);
        let sections = rows(&browser(&keymap, true), "Sections");
        assert!(!sections.iter().any(|r| r.1 == "Copy section as displayed"));
    }

    #[test]
    fn the_quit_row_ends_with_ctrl_c() {
        let content = browser(&Keymap::default(), true);
        let application = rows(&content, "Application");
        assert_eq!(
            application.last().unwrap(),
            &("q / Ctrl-q / Ctrl-w / Ctrl-c".to_owned(), "Quit")
        );
        let keymap = keymap_with("global", "quit", &["x"]);
        let application = rows(&browser(&keymap, true), "Application");
        assert_eq!(application.last().unwrap().0, "x / Ctrl-c");
        let keymap = keymap_with("global", "quit", &[]);
        let application = rows(&browser(&keymap, true), "Application");
        assert_eq!(application.last().unwrap().0, "Ctrl-c");
    }

    #[test]
    fn the_selector_lists_the_session_selector_group_and_application() {
        let content = selector(&ALL);
        assert_eq!(titles(&content), ["Session selector", "Application"]);
        let keymap = Keymap::default();
        let expected: Vec<(String, &str)> = keymap
            .bindings(Context::Selector)
            .iter()
            .map(|b| {
                (
                    keymap.labels(Context::Selector, b.action).unwrap(),
                    b.description,
                )
            })
            .collect();
        assert_eq!(rows(&content, "Session selector"), expected);
        let application = rows(&content, "Application");
        let descriptions: Vec<&str> = application.iter().map(|r| r.1).collect();
        assert_eq!(descriptions, ["Choose theme", "Show keybindings", "Quit"]);
        assert_eq!(application[0].0, "t");
        assert_eq!(application[1].0, "?");
        assert_eq!(application[2].0, "q / Ctrl-q / Ctrl-w / Ctrl-c");
    }

    #[test]
    fn the_selector_lists_only_the_registered_harness_rows() {
        let content = selector(&[HarnessId::Claude, HarnessId::Codex]);
        let descriptions: Vec<&str> = rows(&content, "Session selector")
            .iter()
            .map(|r| r.1)
            .collect();
        assert!(descriptions.contains(&"Switch to Claude"));
        assert!(descriptions.contains(&"Switch to Codex"));
        assert!(!descriptions.contains(&"Switch to OpenCode"));
        assert!(!descriptions.contains(&"Switch to Pi"));
        let all: Vec<&str> = rows(&selector(&ALL), "Session selector")
            .iter()
            .map(|r| r.1)
            .collect();
        assert_eq!(all.len(), descriptions.len() + 2);
        let none: Vec<&str> = rows(&selector(&[]), "Session selector")
            .iter()
            .map(|r| r.1)
            .collect();
        assert!(!none.iter().any(|d| d.starts_with("Switch to")));
    }

    #[test]
    fn the_selector_does_not_list_the_browser_only_rows() {
        let content = selector(&ALL);
        let all: Vec<&str> = content
            .groups()
            .iter()
            .flat_map(|g| &g.rows)
            .map(|r| r.description)
            .collect();
        assert!(!all.contains(&"Open another session"));
        assert!(!all.contains(&"Previous turn"));
    }

    #[test]
    fn the_shift_note_is_a_last_row_only_with_the_mouse_on() {
        let keymap = Keymap::default();
        let with = browser(&keymap, true);
        let without = browser(&keymap, false);
        assert!(with.has_mouse_note() && !without.has_mouse_note());
        let theme = crate::theme::builtin::default_theme();
        let text = |c: &HelpContent| -> Vec<String> {
            c.lines(&theme)
                .iter()
                .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
                .collect()
        };
        assert_eq!(text(&with).last().unwrap().trim(), SHIFT_NOTE);
        assert!(!text(&without).iter().any(|l| l.contains("Shift")));
        assert_eq!(with.row_count(), without.row_count() + 1);
        assert!(!text(&selector(&ALL)).is_empty());
    }

    #[test]
    fn the_row_count_matches_the_lines() {
        let theme = crate::theme::builtin::default_theme();
        for content in [
            browser(&Keymap::default(), true),
            browser(&Keymap::default(), false),
            selector(&ALL),
            selector(&[]),
        ] {
            assert_eq!(content.lines(&theme).len(), content.row_count());
        }
        assert_eq!(browser(&Keymap::default(), true).row_count(), 51);
    }

    #[test]
    fn the_desired_size_is_the_widest_entry_plus_eight_and_the_content_plus_two() {
        let content = browser(&Keymap::default(), true);
        // "q / Ctrl-q / Ctrl-w / Ctrl-c" (28) + 6, then the longest
        // description "Fold or unfold current section" (30) + 8.
        assert_eq!(content.desired_size(), Size::new(28 + 6 + 30 + 8, 53));
    }

    #[test]
    fn the_desired_width_is_at_least_forty() {
        let content = HelpContent {
            groups: vec![HelpGroup {
                title: "G",
                rows: vec![HelpRow {
                    keys: "x".to_owned(),
                    description: "Do",
                }],
            }],
            mouse_note: false,
        };
        assert_eq!(content.desired_size().width, MIN_WIDTH);
    }

    #[test]
    fn entries_are_laid_out_at_the_planned_columns() {
        let theme = crate::theme::builtin::default_theme();
        let lines = browser(&Keymap::default(), true).lines(&theme);
        let text =
            |i: usize| -> String { lines[i].spans.iter().map(|s| s.content.as_ref()).collect() };
        // The interior starts at overlay column 1.
        assert_eq!(text(0), "");
        assert_eq!(text(1), "  Navigation");
        assert_eq!(text(2), "");
        // The keys are padded to the widest label (28) + 6.
        assert_eq!(text(3), format!("    {:<34}Previous section", "Up / k"));
        assert_eq!(text(13), "");
    }
}
