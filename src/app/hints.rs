//! Building the help line's hints and the startup toast text.

use crate::components::Hint;
use crate::config::ConfigWarning;
use crate::keymap::{Action, Context, Keymap};

/// The hints of the session selector, from the active keymap so remapped keys
/// show up: search, open, switch pane, cancel. Actions without a key are left
/// out.
///
/// With the default keys they read `/ to search sessions | ENTER to open |
/// TAB to switch between projects and sessions | ESC to Cancel`.
pub fn selector_hints(keymap: &Keymap) -> Vec<Hint> {
    [
        (Action::Search, "to search sessions"),
        (Action::Open, "to open"),
        (Action::NextPane, "to switch between projects and sessions"),
        (Action::Cancel, "to Cancel"),
    ]
    .into_iter()
    .filter_map(|(action, text)| {
        let key = keymap.label(Context::Selector, action)?;
        Some(Hint::new(key, text))
    })
    .collect()
}

/// The hints of the session browser, from the active keymap so remapped keys
/// show up: search, fold, copy, help. Actions without a key are left out.
///
/// With the default keys they read `/ to search | ENTER to fold or unfold
/// sections | y to copy current | ? for keybindings`.
pub fn browser_hints(keymap: &Keymap) -> Vec<Hint> {
    browser_hints_at(keymap, 0)
}

/// Like [`browser_hints`], for the transcript at subagent `depth`: in a
/// subagent's transcript (depth above 0) `Backspace to go back` comes right
/// after the search hint, where it survives a narrow terminal.
pub fn browser_hints_at(keymap: &Keymap, depth: usize) -> Vec<Hint> {
    let back = (depth > 0).then_some((Context::Browser, Action::Back, "to go back"));
    let search = (Context::Browser, Action::Search, "to search");
    [Some(search), back]
        .into_iter()
        .flatten()
        .chain([
            (
                Context::Browser,
                Action::FoldToggle,
                "to fold or unfold sections",
            ),
            (Context::Browser, Action::Copy, "to copy current"),
            (Context::Global, Action::Help, "for keybindings"),
        ])
        .filter_map(|(context, action, text)| {
            let key = keymap.label(context, action)?;
            Some(Hint::new(key, text))
        })
        .collect()
}

/// The text of the toast that shows the startup `warnings`, or `None` when
/// there are none.
///
/// One warning reads `Config: <warning>`; more read
/// `Config: <first warning> (+N more, see log)`. A warning about a command-line
/// option (its key starts with `--`) is not a config warning and has no
/// `Config: ` prefix: its message names the option.
pub fn startup_toast_text(warnings: &[ConfigWarning]) -> Option<String> {
    let (first, rest) = warnings.split_first()?;
    let first = first
        .message
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let prefix = if warnings[0].key.starts_with("--") {
        ""
    } else {
        "Config: "
    };
    Some(if rest.is_empty() {
        format!("{prefix}{first}")
    } else {
        format!("{prefix}{first} (+{} more, see log)", rest.len())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RawKeys;

    fn warning(message: &str) -> ConfigWarning {
        ConfigWarning {
            key: "k".to_owned(),
            message: message.to_owned(),
        }
    }

    fn keymap_with(table: &str, action: &str, key: &str) -> Keymap {
        let mut raw = RawKeys::new();
        raw.entry(table.to_owned())
            .or_default()
            .insert(action.to_owned(), vec![key.to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        keymap
    }

    fn rendered(hints: &[Hint]) -> String {
        hints
            .iter()
            .map(|h| format!("{} {}", h.key, h.text))
            .collect::<Vec<_>>()
            .join(" | ")
    }

    #[test]
    fn default_selector_hints_match_the_sketch() {
        assert_eq!(
            rendered(&selector_hints(&Keymap::default())),
            "/ to search sessions | ENTER to open | TAB to switch between projects and sessions | ESC to Cancel"
        );
    }

    #[test]
    fn a_remapped_search_key_shows_in_the_first_hint() {
        let hints = selector_hints(&keymap_with("selector", "search", "f"));
        assert_eq!(hints[0], Hint::new("f", "to search sessions"));
        assert_eq!(hints.len(), 4);
    }

    #[test]
    fn remapped_open_next_pane_and_cancel_show_too() {
        let hints = selector_hints(&keymap_with("selector", "open", "o"));
        assert_eq!(hints[1], Hint::new("o", "to open"));
        let hints = selector_hints(&keymap_with("selector", "next_pane", "ctrl-n"));
        assert_eq!(hints[2].key, "Ctrl-n");
        let hints = selector_hints(&keymap_with("selector", "cancel", "x"));
        assert_eq!(hints[3], Hint::new("x", "to Cancel"));
    }

    #[test]
    fn default_browser_hints_read_as_the_help_line_of_the_spec() {
        assert_eq!(
            rendered(&browser_hints(&Keymap::default())),
            "/ to search | ENTER to fold or unfold sections | y to copy current | ? for keybindings"
        );
    }

    #[test]
    fn remapped_browser_keys_show_in_the_hints() {
        let hints = browser_hints(&keymap_with("browser", "search", "f"));
        assert_eq!(hints[0], Hint::new("f", "to search"));
        let hints = browser_hints(&keymap_with("browser", "copy", "c"));
        assert_eq!(hints[2], Hint::new("c", "to copy current"));
        let hints = browser_hints(&keymap_with("browser", "fold_toggle", "space"));
        assert_eq!(hints[1].text, "to fold or unfold sections");
        assert_ne!(hints[1].key, "ENTER");
        let hints = browser_hints(&keymap_with("global", "help", "F1"));
        assert_eq!(hints[3].text, "for keybindings");
        assert_ne!(hints[3].key, "?");
    }

    #[test]
    fn a_subagent_view_adds_the_back_hint_after_search() {
        let keymap = Keymap::default();
        assert_eq!(
            rendered(&browser_hints_at(&keymap, 0)),
            rendered(&browser_hints(&keymap))
        );
        for depth in [1, 2] {
            assert_eq!(
                rendered(&browser_hints_at(&keymap, depth)),
                "/ to search | Backspace to go back | ENTER to fold or unfold sections | y to copy current | ? for keybindings"
            );
        }
    }

    #[test]
    fn the_back_hint_follows_a_remapped_back_key() {
        let keymap = keymap_with("browser", "back", "z");
        assert!(rendered(&browser_hints_at(&keymap, 1)).contains("z to go back"));
    }

    #[test]
    fn no_warnings_no_toast_text() {
        assert_eq!(startup_toast_text(&[]), None);
    }

    #[test]
    fn one_warning_reads_config_colon_warning() {
        assert_eq!(
            startup_toast_text(&[warning("unknown key \"x\"")]).as_deref(),
            Some("Config: unknown key \"x\"")
        );
    }

    #[test]
    fn more_warnings_add_the_count_of_the_rest() {
        let warnings = [warning("first"), warning("second"), warning("third")];
        assert_eq!(
            startup_toast_text(&warnings).as_deref(),
            Some("Config: first (+2 more, see log)")
        );
        assert_eq!(
            startup_toast_text(&warnings[..2]).as_deref(),
            Some("Config: first (+1 more, see log)")
        );
    }

    #[test]
    fn line_breaks_in_a_warning_become_spaces() {
        assert_eq!(
            startup_toast_text(&[warning("bad\nvalue")]).as_deref(),
            Some("Config: bad value")
        );
    }
}
