use std::path::Path;

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::Action::*;
use super::Context::{Browser, Global, Overlay, Selector, Turns};
use super::*;
use crate::config::Config;

const NONE: KeyModifiers = KeyModifiers::NONE;
const CTRL: KeyModifiers = KeyModifiers::CONTROL;
const SHIFT: KeyModifiers = KeyModifiers::SHIFT;

fn ev(code: Key, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

fn ch(c: char) -> KeyEvent {
    ev(Key::Char(c), NONE)
}

/// A keymap from the `[keys]` tables of a config text.
fn keymap(toml: &str) -> (Keymap, Vec<ConfigWarning>) {
    let (config, config_warnings) = Config::from_toml_str(toml, Path::new("/home/tester"));
    assert!(config_warnings.is_empty(), "{config_warnings:?}");
    Keymap::from_config(&config.keys)
}

fn seq(s: &str) -> KeySeq {
    KeySeq::parse(s).unwrap()
}

fn keys_of(km: &Keymap, ctx: Context, action: Action) -> Vec<String> {
    km.keys(ctx, action).iter().map(KeySeq::label).collect()
}

/// (context, name, action, keys, group, description): SPEC planner table.
type Row = (
    Context,
    &'static str,
    Action,
    &'static [&'static str],
    &'static str,
    &'static str,
);

#[rustfmt::skip]
const EXPECTED: &[Row] = &[
    (Global, "quit", Quit, &["q", "ctrl-q", "ctrl-w"], "Application", "Quit"),
    (Global, "help", Help, &["?"], "Application", "Show keybindings"),
    (Global, "theme", Theme, &["t"], "Application", "Choose theme"),
    (Global, "open_selector", OpenSelector, &["ctrl-o"], "Sessions", "Open another session"),
    (Browser, "prev", Prev, &["up", "k"], "Navigation", "Previous section"),
    (Browser, "next", Next, &["down", "j"], "Navigation", "Next section"),
    (Browser, "prev_turn", PrevTurn, &["["], "Navigation", "Previous turn"),
    (Browser, "next_turn", NextTurn, &["]"], "Navigation", "Next turn"),
    (Browser, "half_page_up", HalfPageUp, &["ctrl-u"], "Navigation", "Scroll half a page up"),
    (Browser, "half_page_down", HalfPageDown, &["ctrl-d"], "Navigation", "Scroll half a page down"),
    (Browser, "page_up", PageUp, &["pgup"], "Navigation", "Scroll a page up"),
    (Browser, "page_down", PageDown, &["pgdn"], "Navigation", "Scroll a page down"),
    (Browser, "top", Top, &["g", "home"], "Navigation", "Jump to start"),
    (Browser, "bottom", Bottom, &["G", "end"], "Navigation", "Jump to end and follow"),
    (Browser, "fold_toggle", FoldToggle, &["enter", "space"], "Sections", "Fold or unfold current section"),
    (Browser, "fold", Fold, &["left", "h"], "Sections", "Fold current section"),
    (Browser, "unfold", Unfold, &["right", "l"], "Sections", "Unfold current section"),
    (Browser, "fold_all", FoldAll, &["-"], "Sections", "Fold all sections"),
    (Browser, "unfold_all", UnfoldAll, &["+", "="], "Sections", "Unfold all sections"),
    (Browser, "toggle_system", ToggleSystem, &["s"], "Sections", "Show or hide system content"),
    (Browser, "copy", Copy, &["y"], "Sections", "Copy section as displayed"),
    (Browser, "copy_full", CopyFull, &["Y"], "Sections", "Copy full section"),
    (Browser, "save", Save, &["w", "ctrl-s"], "Sections", "Save section as displayed"),
    (Browser, "save_full", SaveFull, &["W"], "Sections", "Save full section"),
    (Browser, "details", Details, &["i"], "Sections", "Show section details"),
    (Browser, "search", Search, &["/"], "Search", "Search transcript"),
    (Browser, "search_next", SearchNext, &["n"], "Search", "Next match"),
    (Browser, "search_prev", SearchPrev, &["N"], "Search", "Previous match"),
    (Browser, "clear_search", ClearSearch, &["esc"], "Search", "Clear search"),
    (Browser, "open_subagent", OpenSubagent, &["a"], "Sessions", "Open subagent transcript"),
    (Browser, "back", Back, &["backspace"], "Sessions", "Back to parent session"),
    (Browser, "resume", Resume, &["R"], "Sessions", "Resume in harness"),
    (Browser, "export", Export, &["e", "ctrl-e"], "Application", "Export session as folded"),
    (Browser, "export_full", ExportFull, &["E"], "Application", "Export full session"),
    (Browser, "focus_turns", FocusTurns, &["1"], "Turns sidebar", "Focus turns sidebar"),
    (Browser, "next_pane", NextPane, &["tab"], "Turns sidebar", "Next pane"),
    (Browser, "prev_pane", PrevPane, &["shift-tab"], "Turns sidebar", "Previous pane"),
    (Selector, "harness_1", Harness1, &["1"], "Session selector", "Switch to Claude"),
    (Selector, "harness_2", Harness2, &["2"], "Session selector", "Switch to OpenCode"),
    (Selector, "harness_3", Harness3, &["3"], "Session selector", "Switch to Codex"),
    (Selector, "harness_4", Harness4, &["4"], "Session selector", "Switch to Pi"),
    (Selector, "next_pane", NextPane, &["tab"], "Session selector", "Next pane"),
    (Selector, "prev_pane", PrevPane, &["shift-tab"], "Session selector", "Previous pane"),
    (Selector, "focus_projects", FocusProjects, &["left", "h"], "Session selector", "Focus Projects"),
    (Selector, "focus_sessions", FocusSessions, &["right", "l"], "Session selector", "Focus Sessions"),
    (Selector, "up", Up, &["up", "k"], "Session selector", "Move up"),
    (Selector, "down", Down, &["down", "j"], "Session selector", "Move down"),
    (Selector, "search", Search, &["/"], "Session selector", "Search sessions"),
    (Selector, "open", Open, &["enter"], "Session selector", "Open session"),
    (Selector, "cancel", Cancel, &["esc"], "Session selector", "Clear search, close or quit"),
    (Selector, "resume", Resume, &["R"], "Session selector", "Resume in harness"),
    (Overlay, "up", Up, &["up", "k"], "Overlays", "Move up"),
    (Overlay, "down", Down, &["down", "j"], "Overlays", "Move down"),
    (Overlay, "select", Select, &["enter"], "Overlays", "Select"),
    (Overlay, "page_up", PageUp, &["pgup"], "Overlays", "Scroll a page up"),
    (Overlay, "page_down", PageDown, &["pgdn"], "Overlays", "Scroll a page down"),
    (Overlay, "close", Close, &["esc"], "Overlays", "Close"),
    (Turns, "up", Up, &["up", "k"], "Turns sidebar", "Previous entry"),
    (Turns, "down", Down, &["down", "j"], "Turns sidebar", "Next entry"),
    (Turns, "open", Open, &["enter"], "Turns sidebar", "Go to block"),
    (Turns, "next_pane", NextPane, &["tab"], "Turns sidebar", "Next pane"),
    (Turns, "prev_pane", PrevPane, &["shift-tab"], "Turns sidebar", "Previous pane"),
    (Turns, "focus_transcript", FocusTranscript, &["right", "l"], "Turns sidebar", "Focus transcript"),
];

// ---- defaults ----

#[test]
fn default_keymap_matches_the_planner_table() {
    let km = Keymap::default();
    for ctx in Context::ALL {
        let expected: Vec<&Row> = EXPECTED.iter().filter(|r| r.0 == ctx).collect();
        let actual = km.bindings(ctx);
        assert_eq!(actual.len(), expected.len(), "action count in {ctx:?}");
        for (b, row) in actual.iter().zip(expected) {
            let (_, name, action, keys, group, description) = *row;
            assert_eq!(b.name, name, "{ctx:?} order");
            assert_eq!(b.action, action, "{ctx:?} {name}");
            let want: Vec<KeySeq> = keys.iter().map(|k| seq(k)).collect();
            assert_eq!(b.keys, want, "{ctx:?} {name} keys");
            assert_eq!(b.group, group, "{ctx:?} {name} group");
            assert_eq!(b.description, description, "{ctx:?} {name} description");
        }
    }
}

#[test]
fn no_default_is_a_chord_or_modifier_enter() {
    let km = Keymap::default();
    for ctx in Context::ALL {
        for b in km.bindings(ctx) {
            assert!(!b.keys.is_empty(), "{ctx:?} {} has no default", b.name);
            for k in &b.keys {
                assert!(!k.is_chord(), "{ctx:?} {} binds chord {k}", b.name);
                let e = k.first().to_event();
                assert!(
                    e.code != Key::Enter || e.modifiers == NONE,
                    "{ctx:?} {} binds modifier+Enter",
                    b.name
                );
            }
        }
    }
}

#[test]
fn defaults_do_not_collide_within_or_across_contexts() {
    let km = Keymap::default();
    for ctx in Context::ALL {
        let mut seen: Vec<KeySeq> = Vec::new();
        for k in km.bindings(ctx).iter().flat_map(|b| &b.keys) {
            assert!(!seen.contains(k), "{ctx:?}: {k} bound twice");
            seen.push(*k);
        }
    }
    for ctx in [Browser, Selector, Overlay, Turns] {
        for k in km.bindings(ctx).iter().flat_map(|b| &b.keys) {
            for g in km.bindings(Global).iter().flat_map(|b| &b.keys) {
                assert_ne!(k, g, "{ctx:?} collides with global");
            }
        }
    }
}

#[test]
fn default_config_gives_the_default_keymap_without_warnings() {
    let (km, warnings) = Keymap::from_config(&RawKeys::new());
    assert!(warnings.is_empty());
    assert_eq!(km, Keymap::default());
}

#[test]
fn the_spec_example_keeps_the_defaults() {
    let (km, warnings) = keymap(
        r#"
        [keys.browser]
        fold_toggle = ["enter", "space"]
        copy = "y"
        copy_full = "Y"

        [keys.selector]
        search = "/"
        "#,
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(km, Keymap::default());
}

// ---- syntax ----

#[test]
fn parser_accepts_every_documented_form() {
    for s in [
        "a",
        "A",
        "ctrl-d",
        "shift-tab",
        "enter",
        "esc",
        "pgdn",
        "f5",
        "g g",
        "Ctrl-D",
        "ALT-x",
        "ctrl-alt-x",
        "space",
        "backspace",
        "tab",
        "up",
        "down",
        "left",
        "right",
        "home",
        "end",
        "pgup",
        "delete",
        "insert",
        "f1",
        "f12",
        "ENTER",
        "F5",
        "-",
        "ctrl--",
        "+",
        "=",
        "?",
        "[",
        "ctrl-shift-tab",
        "alt-enter",
    ] {
        assert!(KeySeq::parse(s).is_ok(), "{s:?} should parse");
    }
}

#[test]
fn parser_rejects_bad_keys_naming_the_string() {
    for s in [
        "ctrl-", "foo", "f13", "a b c", "", "f0", "g  g", " g", "f05", "shift-",
    ] {
        let err = KeySeq::parse(s).unwrap_err();
        assert_eq!(err.input, s);
        assert!(
            err.to_string().contains(&format!("{s:?}")),
            "{err} should name {s:?}"
        );
    }
}

#[test]
fn single_characters_are_case_sensitive_and_shift_uppercases() {
    assert_ne!(seq("y"), seq("Y"));
    assert_eq!(seq("shift-g"), seq("G"));
    assert_eq!(seq("shift-tab").first().to_event(), ev(Key::BackTab, NONE));
    assert_eq!(seq("CTRL-d"), seq("ctrl-d"));
}

#[test]
fn chords_have_two_keys() {
    let c = seq("g g");
    assert!(c.is_chord());
    assert_eq!(c.first(), c.second().unwrap());
    assert!(!seq("g").is_chord());
}

// ---- resolution ----

fn resolve(ctx: Context, e: KeyEvent) -> Option<Action> {
    Keymap::default().resolve(ctx, &e)
}

#[test]
fn capital_g_resolves_to_bottom_with_or_without_shift() {
    assert_eq!(resolve(Browser, ch('G')), Some(Bottom));
    assert_eq!(resolve(Browser, ev(Key::Char('G'), SHIFT)), Some(Bottom));
    assert_eq!(resolve(Browser, ch('g')), Some(Top));
}

#[test]
fn shifted_punctuation_resolves_with_or_without_shift() {
    assert_eq!(resolve(Global, ev(Key::Char('?'), SHIFT)), Some(Help));
    assert_eq!(resolve(Global, ch('?')), Some(Help));
    assert_eq!(resolve(Browser, ev(Key::Char('+'), SHIFT)), Some(UnfoldAll));
}

#[test]
fn back_tab_resolves_to_prev_pane() {
    assert_eq!(resolve(Selector, ev(Key::BackTab, NONE)), Some(PrevPane));
    assert_eq!(resolve(Selector, ev(Key::BackTab, SHIFT)), Some(PrevPane));
    assert_eq!(resolve(Selector, ev(Key::Tab, SHIFT)), Some(PrevPane));
    assert_eq!(resolve(Selector, ev(Key::Tab, NONE)), Some(NextPane));
}

#[test]
fn space_resolves_to_fold_toggle() {
    assert_eq!(resolve(Browser, ch(' ')), Some(FoldToggle));
    assert_eq!(resolve(Browser, ev(Key::Enter, NONE)), Some(FoldToggle));
}

#[test]
fn ctrl_d_resolves_to_half_page_down() {
    assert_eq!(
        resolve(Browser, ev(Key::Char('d'), CTRL)),
        Some(HalfPageDown)
    );
    assert_eq!(resolve(Browser, ch('d')), None);
}

#[test]
fn ctrl_c_resolves_to_nothing_in_every_context() {
    let km = Keymap::default();
    for ctx in Context::ALL {
        assert_eq!(km.resolve(ctx, &ev(Key::Char('c'), CTRL)), None, "{ctx:?}");
        let mut r = KeyResolver::new(ctx);
        assert_eq!(
            r.feed(&km, &ev(Key::Char('c'), CTRL)),
            Resolved::Unbound,
            "{ctx:?}"
        );
    }
}

#[test]
fn contexts_resolve_independently() {
    assert_eq!(resolve(Browser, ch('j')), Some(Next));
    assert_eq!(resolve(Selector, ch('j')), Some(Down));
    assert_eq!(resolve(Overlay, ch('j')), Some(Down));
    assert_eq!(resolve(Global, ch('j')), None);
    assert_eq!(resolve(Global, ch('q')), Some(Quit));
    assert_eq!(
        resolve(Global, ev(Key::Char('o'), CTRL)),
        Some(OpenSelector)
    );
    assert_eq!(resolve(Global, ch('o')), None);
    assert_eq!(resolve(Selector, ch('3')), Some(Harness3));
    assert_eq!(resolve(Browser, ch('3')), None);
    assert_eq!(resolve(Browser, ev(Key::PageDown, NONE)), Some(PageDown));
    assert_eq!(resolve(Browser, ev(Key::Home, NONE)), Some(Top));
}

#[test]
fn unlisted_keys_and_modifiers_do_not_resolve() {
    assert_eq!(resolve(Browser, ev(Key::Char('b'), CTRL)), None);
    assert_eq!(resolve(Browser, ev(Key::Char('f'), CTRL)), None);
    assert_eq!(
        resolve(Browser, ev(Key::Char('y'), KeyModifiers::ALT)),
        None
    );
    assert_eq!(resolve(Browser, ev(Key::Char('z'), NONE)), None);
}

// ---- overrides ----

#[test]
fn override_replaces_the_default_keys_entirely() {
    let (km, warnings) = keymap("[keys.browser]\ncopy = \"c\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(km.resolve(Browser, &ch('c')), Some(Copy));
    assert_eq!(km.resolve(Browser, &ch('y')), None);
}

#[test]
fn override_with_several_keys_and_other_contexts() {
    let (km, warnings) = keymap(
        "[keys.browser]\ncopy = [\"c\", \"ctrl-y\"]\n[keys.selector]\nsearch = \"f\"\n[keys.overlay]\nclose = \"x\"\n[keys.global]\nhelp = \"f1\"\n",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(km.resolve(Browser, &ev(Key::Char('y'), CTRL)), Some(Copy));
    assert_eq!(km.resolve(Selector, &ch('f')), Some(Search));
    assert_eq!(km.resolve(Selector, &ch('/')), None);
    assert_eq!(km.resolve(Browser, &ch('/')), Some(Search));
    assert_eq!(km.resolve(Overlay, &ch('x')), Some(Close));
    assert_eq!(km.resolve(Global, &ev(Key::Function(1), NONE)), Some(Help));
    assert_eq!(km.resolve(Global, &ch('?')), None);
}

#[test]
fn empty_list_unbinds_an_action_without_warning() {
    let (km, warnings) = keymap("[keys.browser]\ncopy = []\n");
    assert!(warnings.is_empty());
    assert_eq!(km.resolve(Browser, &ch('y')), None);
    assert_eq!(km.label(Browser, Copy), None);
    assert!(
        km.help_entries(Browser)
            .iter()
            .all(|e| e.description != "Copy section as displayed")
    );
}

fn assert_default_with_one_warning(toml: &str, key: &str, needle: &str) {
    let (km, warnings) = keymap(toml);
    assert_eq!(km, Keymap::default(), "defaults kept for {toml}");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].key, key);
    assert!(
        warnings[0].message.contains(needle),
        "{} should contain {needle}",
        warnings[0]
    );
}

#[test]
fn unknown_action_warns_and_keeps_defaults() {
    assert_default_with_one_warning(
        "[keys.browser]\nfrobnicate = \"x\"\n",
        "keys.browser.frobnicate",
        "frobnicate",
    );
}

#[test]
fn action_of_another_context_is_unknown() {
    assert_default_with_one_warning(
        "[keys.browser]\nharness_1 = \"x\"\n",
        "keys.browser.harness_1",
        "harness_1",
    );
}

#[test]
fn unknown_context_warns_and_keeps_defaults() {
    assert_default_with_one_warning("[keys.nowhere]\ncopy = \"x\"\n", "keys.nowhere", "nowhere");
}

#[test]
fn the_warning_for_an_unknown_context_lists_the_known_ones_with_turns() {
    let (_, warnings) = keymap("[keys.nowhere]\ncopy = \"x\"\n");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0]
            .message
            .contains("(known: global, browser, selector, overlay, turns)"),
        "{}",
        warnings[0]
    );
}

#[test]
fn unparsable_key_warns_naming_it_and_keeps_the_default() {
    assert_default_with_one_warning(
        "[keys.browser]\ncopy = \"foo\"\n",
        "keys.browser.copy",
        "\"foo\"",
    );
    // One bad key in a list rejects the whole entry.
    assert_default_with_one_warning(
        "[keys.browser]\ncopy = [\"c\", \"f13\"]\n",
        "keys.browser.copy",
        "\"f13\"",
    );
}

#[test]
fn chord_in_global_or_overlay_warns_and_keeps_the_default() {
    assert_default_with_one_warning(
        "[keys.global]\nquit = \"g g\"\n",
        "keys.global.quit",
        "chord",
    );
    assert_default_with_one_warning(
        "[keys.overlay]\nclose = \"g g\"\n",
        "keys.overlay.close",
        "chord",
    );
}

#[test]
fn collision_with_a_global_key_warns_and_keeps_the_default() {
    assert_default_with_one_warning(
        "[keys.browser]\ncopy = \"t\"\n",
        "keys.browser.copy",
        "theme",
    );
    assert_default_with_one_warning(
        "[keys.selector]\nsearch = \"ctrl-o\"\n",
        "keys.selector.search",
        "open_selector",
    );
    // Also when the global key is the second key of a chord.
    assert_default_with_one_warning(
        "[keys.browser]\ncopy = \"g t\"\n",
        "keys.browser.copy",
        "theme",
    );
}

#[test]
fn collision_check_uses_the_user_set_global_keys() {
    let (km, warnings) =
        keymap("[keys.global]\ntheme = \"T\"\n[keys.browser]\ncopy = [\"t\", \"T\"]\n");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].key, "keys.browser.copy");
    assert_eq!(km.resolve(Browser, &ch('y')), Some(Copy));
    assert_eq!(km.resolve(Global, &ch('T')), Some(Theme));
}

#[test]
fn a_user_global_key_takes_a_default_from_another_context() {
    let (km, warnings) = keymap("[keys.global]\nhelp = \"n\"\n");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].key, "keys.browser.search_next");
    assert_eq!(km.resolve(Global, &ch('n')), Some(Help));
    assert_eq!(km.resolve(Browser, &ch('n')), None);
}

#[test]
fn ctrl_c_cannot_be_bound() {
    assert_default_with_one_warning(
        "[keys.browser]\ncopy = \"ctrl-c\"\n",
        "keys.browser.copy",
        "Ctrl-c",
    );
    assert_default_with_one_warning(
        "[keys.global]\nquit = [\"q\", \"ctrl-c\"]\n",
        "keys.global.quit",
        "Ctrl-c",
    );
}

#[test]
fn same_context_collision_moves_the_key_and_warns() {
    let (km, warnings) = keymap("[keys.browser]\ncopy = \"n\"\n");
    assert_eq!(km.resolve(Browser, &ch('n')), Some(Copy));
    assert!(keys_of(&km, Browser, SearchNext).is_empty());
    assert_eq!(keys_of(&km, Browser, Copy), ["n"]);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].key, "keys.browser.copy");
    assert!(warnings[0].message.contains("search_next"));
    // Nothing else changed.
    assert_eq!(keys_of(&km, Browser, SearchPrev), ["N"]);
}

#[test]
fn collision_removes_only_the_shared_key() {
    let (km, warnings) = keymap("[keys.browser]\nsave = \"k\"\n");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(keys_of(&km, Browser, Prev), ["Up"]);
    assert_eq!(km.resolve(Browser, &ch('k')), Some(Save));
}

#[test]
fn same_key_in_different_contexts_is_fine() {
    let (km, warnings) = keymap("[keys.browser]\ncopy = \"2\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(km.resolve(Browser, &ch('2')), Some(Copy));
    assert_eq!(km.resolve(Selector, &ch('2')), Some(Harness2));
}

#[test]
fn each_bad_entry_is_skipped_independently() {
    let (km, warnings) = keymap(
        "[keys.browser]\ncopy = \"c\"\nsave = \"foo\"\nbogus = \"x\"\n[keys.selector]\nsearch = \"s\"\n",
    );
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert_eq!(km.resolve(Browser, &ch('c')), Some(Copy));
    assert_eq!(km.resolve(Browser, &ch('w')), Some(Save));
    assert_eq!(km.resolve(Selector, &ch('s')), Some(Search));
}

// ---- chords ----

fn feed(r: &mut KeyResolver, km: &Keymap, c: char) -> Resolved {
    r.feed(km, &ch(c))
}

#[test]
fn chord_completes_with_its_second_key() {
    let (km, warnings) = keymap("[keys.browser]\ntop = \"g g\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut r = KeyResolver::new(Browser);
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Pending);
    assert!(r.is_pending());
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Action(Top));
    assert!(!r.is_pending());
    // Starts over.
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Pending);
}

#[test]
fn completes_is_about_the_chord_and_not_about_the_key_alone() {
    let (km, warnings) = keymap("[keys.browser]\nsearch = \"z x\"\ntop = \"v\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut r = KeyResolver::new(Browser);
    assert!(!r.completes(&km, &ch('x'), Search), "nothing is pending");
    assert_eq!(feed(&mut r, &km, 'z'), Resolved::Pending);
    assert!(r.completes(&km, &ch('x'), Search));
    assert!(!r.completes(&km, &ch('x'), Top), "another action");
    assert!(!r.completes(&km, &ch('v'), Top), "a single key");
    assert!(r.is_pending(), "asking changes nothing");
}

#[test]
fn a_key_that_does_not_complete_the_chord_is_resolved_afresh() {
    let (km, _) = keymap("[keys.browser]\ntop = \"g g\"\n");
    let mut r = KeyResolver::new(Browser);
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Pending);
    assert_eq!(feed(&mut r, &km, 'j'), Resolved::Action(Next));
    assert!(!r.is_pending());
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Pending);
    assert_eq!(feed(&mut r, &km, 'z'), Resolved::Unbound);
    assert!(!r.is_pending());
}

#[test]
fn a_chord_prefix_is_only_a_prefix() {
    let (km, warnings) = keymap("[keys.browser]\ntop = \"g g\"\n");
    assert!(
        warnings.is_empty(),
        "the replaced default is not a conflict"
    );
    assert_eq!(
        km.resolve(Browser, &ch('g')),
        None,
        "single-key resolve skips prefixes"
    );

    let (km, warnings) = keymap("[keys.browser]\nsearch = \"g g\"\n");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].key, "keys.browser.top");
    assert_eq!(keys_of(&km, Browser, Top), ["Home"]);
    let mut r = KeyResolver::new(Browser);
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Pending);
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Action(Search));
}

#[test]
fn single_binding_of_a_prefix_key_in_the_same_entry_is_dropped() {
    let (km, warnings) = keymap("[keys.browser]\ntop = [\"g\", \"g g\"]\n");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(keys_of(&km, Browser, Top), ["g g"]);
}

#[test]
fn a_chord_in_the_selector_works_and_contexts_are_separate() {
    let (km, warnings) = keymap("[keys.selector]\nsearch = \"z z\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut selector = KeyResolver::new(Selector);
    assert_eq!(feed(&mut selector, &km, 'z'), Resolved::Pending);
    assert_eq!(feed(&mut selector, &km, 'z'), Resolved::Action(Search));
    let mut browser = KeyResolver::new(Browser);
    assert_eq!(feed(&mut browser, &km, 'z'), Resolved::Unbound);
}

#[test]
fn resolver_matches_single_keys_like_the_keymap() {
    let km = Keymap::default();
    let mut r = KeyResolver::new(Browser);
    assert_eq!(feed(&mut r, &km, 'G'), Resolved::Action(Bottom));
    assert_eq!(
        r.feed(&km, &ev(Key::Char('G'), SHIFT)),
        Resolved::Action(Bottom)
    );
    assert_eq!(feed(&mut r, &km, 'z'), Resolved::Unbound);
}

// ---- labels and help ----

#[test]
fn labels_use_the_documented_formats() {
    let km = Keymap::default();
    assert_eq!(km.labels(Browser, Save).as_deref(), Some("w / Ctrl-s"));
    assert_eq!(km.labels(Browser, Prev).as_deref(), Some("Up / k"));
    assert_eq!(km.label(Selector, NextPane).as_deref(), Some("TAB"));
    assert_eq!(km.label(Selector, PrevPane).as_deref(), Some("Shift-TAB"));
    assert_eq!(km.label(Browser, Copy).as_deref(), Some("y"));
    assert_eq!(km.label(Browser, CopyFull).as_deref(), Some("Y"));
    assert_eq!(km.label(Browser, Search).as_deref(), Some("/"));
    assert_eq!(km.label(Global, Help).as_deref(), Some("?"));
    assert_eq!(
        km.labels(Browser, FoldToggle).as_deref(),
        Some("ENTER / SPACE")
    );
    assert_eq!(km.label(Browser, ClearSearch).as_deref(), Some("ESC"));
    assert_eq!(km.label(Browser, Back).as_deref(), Some("Backspace"));
    assert_eq!(km.labels(Browser, Top).as_deref(), Some("g / Home"));
    assert_eq!(km.labels(Browser, Bottom).as_deref(), Some("G / End"));
    assert_eq!(km.labels(Browser, PageDown).as_deref(), Some("PgDn"));
    assert_eq!(km.labels(Browser, PageUp).as_deref(), Some("PgUp"));
    assert_eq!(km.labels(Browser, Fold).as_deref(), Some("Left / h"));
    assert_eq!(
        km.labels(Global, Quit).as_deref(),
        Some("q / Ctrl-q / Ctrl-w")
    );
    assert_eq!(km.labels(Global, OpenSelector).as_deref(), Some("Ctrl-o"));
}

#[test]
fn every_named_key_has_its_documented_label() {
    for (key, label) in [
        ("enter", "ENTER"),
        ("esc", "ESC"),
        ("tab", "TAB"),
        ("shift-tab", "Shift-TAB"),
        ("space", "SPACE"),
        ("backspace", "Backspace"),
        ("up", "Up"),
        ("down", "Down"),
        ("left", "Left"),
        ("right", "Right"),
        ("home", "Home"),
        ("end", "End"),
        ("pgup", "PgUp"),
        ("pgdn", "PgDn"),
        ("delete", "Delete"),
        ("insert", "Insert"),
        ("f5", "F5"),
        ("ctrl-x", "Ctrl-x"),
        ("alt-x", "Alt-x"),
        ("ctrl-alt-x", "Ctrl-Alt-x"),
        ("shift-up", "Shift-Up"),
        ("g g", "g g"),
        ("shift-g", "G"),
    ] {
        assert_eq!(seq(key).label(), label, "{key}");
    }
}

#[test]
fn labels_reflect_remappings() {
    let (km, warnings) = keymap(
        "[keys.browser]\nsave = [\"ctrl-s\", \"W\"]\ntop = \"g g\"\n[keys.selector]\nnext_pane = \"ctrl-n\"\n",
    );
    assert!(
        warnings.iter().all(|w| w.message.contains("save_full")),
        "{warnings:?}"
    );
    assert_eq!(km.labels(Browser, Save).as_deref(), Some("Ctrl-s / W"));
    assert_eq!(km.label(Browser, Save).as_deref(), Some("Ctrl-s"));
    assert_eq!(km.label(Browser, Top).as_deref(), Some("g g"));
    assert_eq!(km.label(Selector, NextPane).as_deref(), Some("Ctrl-n"));
    assert_eq!(km.label(Selector, PrevPane).as_deref(), Some("Shift-TAB"));
}

#[test]
fn help_entries_follow_table_order_and_groups() {
    let km = Keymap::default();
    let entries = km.help_entries(Browser);
    let expected: Vec<&Row> = EXPECTED.iter().filter(|r| r.0 == Browser).collect();
    assert_eq!(entries.len(), expected.len());
    for (e, row) in entries.iter().zip(&expected) {
        assert_eq!(e.group, row.4);
        assert_eq!(e.description, row.5);
        assert_eq!(Some(e.keys.clone()), km.labels(Browser, row.2));
    }
    let mut groups: Vec<&str> = Vec::new();
    for e in &entries {
        if groups.last() != Some(&e.group) {
            groups.push(e.group);
        }
    }
    assert_eq!(
        groups,
        [
            "Navigation",
            "Sections",
            "Search",
            "Sessions",
            "Application",
            "Turns sidebar"
        ]
    );
    assert_eq!(entries[0].keys, "Up / k");
    assert_eq!(entries[0].description, "Previous section");
    let save = entries
        .iter()
        .find(|e| e.description == "Save section as displayed")
        .unwrap();
    assert_eq!(save.keys, "w / Ctrl-s");
}

#[test]
fn help_entries_show_remapped_keys() {
    let (km, _) = keymap("[keys.browser]\ncopy = \"c\"\n");
    let entries = km.help_entries(Browser);
    let copy = entries
        .iter()
        .find(|e| e.description == "Copy section as displayed")
        .unwrap();
    assert_eq!(copy.keys, "c");
    assert_eq!(copy.group, "Sections");
    assert!(entries.iter().all(|e| e.keys != "y"));
}

#[test]
fn the_details_key_is_remappable_and_resolves_to_the_action() {
    let km = Keymap::default();
    assert_eq!(
        km.resolve(Browser, &seq("i").first().to_event()),
        Some(Details)
    );
    let (km, warnings) = keymap("[keys.browser]\ndetails = [\"x\", \"ctrl-i\"]\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(
        km.resolve(Browser, &seq("x").first().to_event()),
        Some(Details)
    );
    assert_eq!(km.resolve(Browser, &seq("i").first().to_event()), None);
    assert_eq!(km.labels(Browser, Details).as_deref(), Some("x / Ctrl-i"));
    let entries = km.help_entries(Browser);
    let details = entries
        .iter()
        .find(|e| e.description == "Show section details")
        .unwrap();
    assert_eq!(details.keys, "x / Ctrl-i");
    assert_eq!(details.group, "Sections");
}

#[test]
fn help_entries_exist_for_every_context() {
    let km = Keymap::default();
    for ctx in Context::ALL {
        let rows = EXPECTED.iter().filter(|r| r.0 == ctx).count();
        assert_eq!(km.help_entries(ctx).len(), rows);
    }
    let global = km.help_entries(Global);
    assert_eq!(global[0].keys, "q / Ctrl-q / Ctrl-w");
    assert_eq!(global[3].group, "Sessions");
}

#[test]
fn context_names_round_trip() {
    for ctx in Context::ALL {
        assert_eq!(Context::from_name(ctx.name()), Some(ctx));
    }
    assert_eq!(Context::from_name("nope"), None);
}

// ---- the turns sidebar ----

#[test]
fn one_and_tab_act_on_the_selector_there_and_focus_the_sidebar_in_the_browser() {
    let tab = ev(Key::Tab, NONE);
    let back = ev(Key::BackTab, NONE);
    // The selector keeps them: `1` switches harness, `Tab` switches pane.
    assert_eq!(resolve(Selector, ch('1')), Some(Harness1));
    assert_eq!(resolve(Selector, tab), Some(NextPane));
    assert_eq!(resolve(Selector, back), Some(PrevPane));
    // The browser focuses the sidebar with them.
    assert_eq!(resolve(Browser, ch('1')), Some(FocusTurns));
    assert_eq!(resolve(Browser, tab), Some(NextPane));
    assert_eq!(resolve(Browser, back), Some(PrevPane));
    // Nothing leaks the other way.
    assert_eq!(resolve(Selector, ch('l')), Some(FocusSessions));
    assert_eq!(resolve(Browser, ch('l')), Some(Unfold));
    assert_eq!(resolve(Turns, ch('l')), Some(FocusTranscript));
}

#[test]
fn the_sidebar_keys_are_the_selectors_and_one_and_left_do_nothing_there() {
    let key = |code| ev(code, NONE);
    assert_eq!(resolve(Turns, key(Key::Up)), Some(Up));
    assert_eq!(resolve(Turns, ch('k')), Some(Up));
    assert_eq!(resolve(Turns, key(Key::Down)), Some(Down));
    assert_eq!(resolve(Turns, ch('j')), Some(Down));
    assert_eq!(resolve(Turns, key(Key::Enter)), Some(Open));
    assert_eq!(resolve(Turns, key(Key::Tab)), Some(NextPane));
    assert_eq!(resolve(Turns, key(Key::BackTab)), Some(PrevPane));
    assert_eq!(resolve(Turns, key(Key::Right)), Some(FocusTranscript));
    assert_eq!(resolve(Turns, ch('1')), None);
    assert_eq!(resolve(Turns, key(Key::Left)), None);
    assert_eq!(resolve(Turns, ch('h')), None);
    assert_eq!(resolve(Turns, ch('/')), None);
}

#[test]
fn the_turns_context_can_be_remapped_like_any_other() {
    let (km, warnings) = keymap(
        "[keys.turns]\nup = \"w\"\ndown = [\"s\", \"ctrl-n\"]\nopen = \"o\"\nfocus_transcript = \"x\"\nnext_pane = \"n\"\n[keys.browser]\nfocus_turns = \"f\"\n",
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(km.resolve(Turns, &ch('w')), Some(Up));
    assert_eq!(km.resolve(Turns, &ch('k')), None, "the default is replaced");
    assert_eq!(km.resolve(Turns, &ch('s')), Some(Down));
    assert_eq!(km.resolve(Turns, &ev(Key::Char('n'), CTRL)), Some(Down));
    assert_eq!(km.resolve(Turns, &ch('o')), Some(Open));
    assert_eq!(km.resolve(Turns, &ch('x')), Some(FocusTranscript));
    assert_eq!(km.resolve(Turns, &ch('n')), Some(NextPane));
    assert_eq!(km.resolve(Turns, &ev(Key::Tab, NONE)), None);
    assert_eq!(km.resolve(Browser, &ch('f')), Some(FocusTurns));
    assert_eq!(km.resolve(Browser, &ch('1')), None);
    // The other contexts are untouched.
    assert_eq!(km.resolve(Selector, &ch('1')), Some(Harness1));
    assert_eq!(km.resolve(Selector, &ch('k')), Some(Up));
    assert_eq!(km.labels(Turns, Down).as_deref(), Some("s / Ctrl-n"));
    assert_eq!(km.label(Browser, FocusTurns).as_deref(), Some("f"));
}

#[test]
fn a_turns_binding_that_collides_with_a_global_key_is_refused() {
    assert_default_with_one_warning("[keys.turns]\nup = \"t\"\n", "keys.turns.up", "theme");
    let (km, warnings) = keymap("[keys.global]\nhelp = \"j\"\n");
    assert!(
        warnings.iter().any(|w| w.key == "keys.turns.down"),
        "{warnings:?}"
    );
    assert_eq!(km.resolve(Turns, &ch('j')), None);
}

#[test]
fn a_chord_in_the_turns_context_works() {
    let (km, warnings) = keymap("[keys.turns]\nopen = \"g g\"\n");
    assert!(warnings.is_empty(), "{warnings:?}");
    let mut r = KeyResolver::new(Turns);
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Pending);
    assert_eq!(feed(&mut r, &km, 'g'), Resolved::Action(Open));
}

#[test]
fn an_action_of_the_browser_is_unknown_in_the_turns_context_and_the_other_way_round() {
    assert_default_with_one_warning(
        "[keys.turns]\nfocus_turns = \"x\"\n",
        "keys.turns.focus_turns",
        "focus_turns",
    );
    assert_default_with_one_warning(
        "[keys.browser]\nfocus_transcript = \"x\"\n",
        "keys.browser.focus_transcript",
        "focus_transcript",
    );
}
