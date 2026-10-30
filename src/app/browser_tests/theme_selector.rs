//! The theme selector in the app (SPEC §8.4): opening, previewing on every
//! mounted component, keeping and cancelling, saving to the config file, and
//! the screens. Keys travel through the app's ports and `GlobalEvents`, as
//! they do in the running application.

use ratatui::style::Color;
use tuirealm::state::{State, StateValue};

use super::super::selector_tests::truecolor;
use super::*;
use crate::components::{THEME_ATTR, theme_from_value};
use crate::theme::{TerminalEnv, ThemeRegistry, resolve_by_id};

/// Every component that draws with the theme. The match below is exhaustive
/// on purpose: a new `Id` makes this fail to compile until it is placed here
/// and, if it draws, in `THEMED` of `app/theme_selector.rs`.
const THEMED_IDS: [Id; 15] = [
    Id::SelectorFrame,
    Id::EmptyState,
    Id::HarnessTabs,
    Id::ProjectList,
    Id::SessionList,
    Id::WorkingDir,
    Id::HelpLine,
    Id::BrowserHeader,
    Id::Transcript,
    Id::BrowserHelpLine,
    Id::EmptySession,
    Id::Prompt,
    Id::HelpOverlay,
    Id::ThemeSelector,
    Id::Toast,
];

#[test]
fn every_id_is_themed_but_the_global_events() {
    for id in THEMED_IDS {
        let themed = match id {
            Id::GlobalEvents => false,
            Id::SelectorFrame
            | Id::EmptyState
            | Id::HarnessTabs
            | Id::ProjectList
            | Id::SessionList
            | Id::WorkingDir
            | Id::HelpLine
            | Id::BrowserHeader
            | Id::Transcript
            | Id::Subagent(_)
            | Id::BrowserHelpLine
            | Id::EmptySession
            | Id::Prompt
            | Id::HelpOverlay
            | Id::ThemeSelector
            | Id::Toast => true,
        };
        assert!(themed, "{id:?}");
    }
}

/// The theme of each mounted component that has one.
pub(super) fn mounted_themes(app: &App) -> Vec<(Id, Theme)> {
    let mut ids = THEMED_IDS.to_vec();
    ids.extend((1..=app.subagent_depth()).map(Id::Subagent));
    ids.into_iter()
        .filter(|id| app.application().mounted(id))
        .map(|id| {
            let value = app
                .application()
                .query(&id, THEME_ATTR)
                .unwrap()
                .unwrap_or_else(|| panic!("{id:?} does not answer for its theme"))
                .into_attr();
            let theme = theme_from_value(&value).unwrap_or_else(|| panic!("{id:?}"));
            (id, theme)
        })
        .collect()
}

/// Checks that `at_least` components are mounted and every one has `expected`.
pub(super) fn assert_all_have(app: &App, expected: &Theme, at_least: usize) {
    let all = mounted_themes(app);
    assert!(all.len() >= at_least, "only {all:?}");
    for (id, theme) in &all {
        assert_eq!(theme, expected, "{id:?}");
    }
}

fn init_on(active: &str) -> AppInit {
    let mut init = init(&[CLAUDE], theme(active));
    init.theme_id = active.to_owned();
    init
}

fn rig_on(active: &str) -> Rig {
    build_rig_from(init_on(active), 120, 40, false)
}

/// A complete user theme document named `name`.
fn theme_toml(name: &str) -> String {
    let mut out = format!("name = \"{name}\"\n");
    for (i, role) in Role::ALL.iter().enumerate() {
        let v = u8::try_from(i).unwrap() * 4 + 20;
        out.push_str(&format!("{} = \"#{v:02x}{v:02x}{v:02x}\"\n", role.key()));
    }
    out
}

/// The built-ins and the user themes `(id, name)`; a name of `None` writes a
/// theme that does not parse.
fn registry_with(user: &[(&str, Option<&str>)]) -> ThemeRegistry {
    let dir = tempfile::tempdir().unwrap();
    for (id, name) in user {
        let text = match name {
            Some(name) => theme_toml(name),
            None => "name = \"broken\"\nforeground = \"not a colour\"\n".to_owned(),
        };
        std::fs::write(dir.path().join(format!("{id}.toml")), text).unwrap();
    }
    ThemeRegistry::load(dir.path())
}

fn rig_with_registry(registry: ThemeRegistry, width: u16, height: u16) -> Rig {
    let mut init = init_on("default");
    init.registry = registry;
    build_rig_from(init, width, height, false)
}

/// A rig whose config file is `config.toml` in the returned directory.
fn rig_saving(config: Option<&str>) -> (Rig, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.toml");
    if let Some(text) = config {
        std::fs::write(&path, text).unwrap();
    }
    let mut init = init_on("default");
    init.config_path = Some(path);
    (build_rig_from(init, 120, 40, false), dir)
}

fn config_text(dir: &tempfile::TempDir) -> Option<String> {
    std::fs::read_to_string(dir.path().join("config.toml")).ok()
}

impl Rig {
    /// Presses `t` and waits until the selector is up.
    pub(super) fn open_themes(&mut self) {
        self.press(Key::Char('t'));
        self.pump_until(|app| app.application().mounted(&Id::ThemeSelector));
    }

    fn themes_open(&self) -> bool {
        self.app.application().mounted(&Id::ThemeSelector)
    }

    /// The id of the theme under the cursor.
    fn cursor(&self) -> Option<String> {
        match self.app.application().state(&Id::ThemeSelector).unwrap() {
            State::Single(StateValue::String(id)) => Some(id),
            State::None => None,
            other => panic!("{other:?}"),
        }
    }

    /// Presses `code` and waits until the app shows the theme `id`.
    pub(super) fn move_to(&mut self, code: Key, id: &str) {
        self.press(code);
        self.pump_until(|app| app.theme_id() == id);
    }

    pub(super) fn close_themes(&mut self) {
        self.press(Key::Esc);
        self.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    }
}

fn open_at(width: u16, height: u16) -> Rig {
    let mut rig = rig();
    rig.app.update(Msg::Resized(width, height));
    rig.open_loaded();
    rig.open_themes();
    rig
}

// -- opening and closing

#[test]
fn t_in_the_browser_opens_the_selector_and_gives_it_the_focus() {
    let mut rig = rig();
    rig.open_loaded();
    assert_eq!(rig.focus(), Some(Id::Transcript));
    rig.press(Key::Char('t'));
    let seen = rig.pump_until(|app| app.application().mounted(&Id::ThemeSelector));
    assert!(seen.contains(&Msg::OpenThemeSelector), "{seen:?}");
    assert_eq!(rig.focus(), Some(Id::ThemeSelector));
    assert_eq!(rig.app.screen(), Screen::Browser);
    for id in [Id::Transcript, Id::BrowserHeader, Id::BrowserHelpLine] {
        assert!(rig.app.application().mounted(&id), "{id:?}");
    }
}

#[test]
fn t_in_the_startup_selector_opens_it_and_esc_returns_the_focus_to_the_pane() {
    let mut rig = rig();
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    rig.open_themes();
    assert_eq!(rig.focus(), Some(Id::ThemeSelector));
    assert_eq!(rig.app.screen(), Screen::Selector);
    rig.close_themes();
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert_eq!(rig.app.screen(), Screen::Selector);
}

#[test]
fn t_in_the_selector_over_the_browser_returns_the_focus_to_its_pane() {
    let mut rig = rig();
    rig.open_loaded();
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    rig.open_themes();
    assert_eq!(rig.app.screen(), Screen::BrowserSelector);
    rig.close_themes();
    assert_eq!(rig.focus(), Some(Id::SessionList));
    // The next Esc closes the selector, as it did without the overlay.
    rig.press(Key::Esc);
    rig.pump_until(|app| app.screen() == Screen::Browser);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn the_cursor_starts_on_the_active_theme() {
    for id in ["default", "terminal", "gruvbox", "tokyo-night", "solarized"] {
        let mut rig = rig_on(id);
        rig.open_themes();
        assert_eq!(rig.cursor().as_deref(), Some(id));
    }
}

#[test]
fn a_second_open_request_changes_nothing() {
    let mut rig = rig();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.app.update(Msg::OpenThemeSelector);
    assert_eq!(rig.cursor().as_deref(), Some("terminal"));
    assert_eq!(rig.app.theme_id(), "terminal");
    rig.close_themes();
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert_eq!(rig.app.theme_id(), "default");
}

#[test]
fn it_does_not_open_over_the_help_overlay_or_a_prompt() {
    let mut rig = rig();
    rig.press(Key::Char('?'));
    rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    rig.app.update(Msg::OpenThemeSelector);
    assert!(!rig.themes_open());
    assert_eq!(rig.focus(), Some(Id::HelpOverlay));
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::HelpOverlay));
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.app.update(Msg::OpenThemeSelector);
    assert!(!rig.themes_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
}

#[test]
fn t_is_text_in_a_prompt() {
    let mut rig = rig();
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.press(Key::Char('t'));
    rig.settle();
    assert!(!rig.themes_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
}

// -- keys while it is open

#[test]
fn q_and_the_other_global_keys_do_nothing_while_it_is_open() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_themes();
    for code in [
        Key::Char('q'),
        Key::Char('t'),
        Key::Char('?'),
        Key::Char('/'),
    ] {
        rig.press(code);
    }
    rig.ctrl_o();
    rig.ctrl('q');
    let seen = rig.settle();
    assert!(!rig.app.should_quit(), "{seen:?}");
    assert!(
        !seen.iter().any(|m| matches!(
            m,
            Msg::Quit | Msg::OpenSelector | Msg::OpenSearch | Msg::OpenHelp(_)
        )),
        "{seen:?}"
    );
    assert!(rig.themes_open());
    assert_eq!(rig.focus(), Some(Id::ThemeSelector));
    assert_eq!(rig.app.screen(), Screen::Browser);
    assert_eq!(rig.app.theme_id(), "default");
    assert_eq!(rig.cursor().as_deref(), Some("default"));
    // Control: once it is closed the same key quits.
    rig.close_themes();
    rig.press(Key::Char('q'));
    rig.pump_until(App::should_quit);
}

#[test]
fn ctrl_c_quits_from_the_selector() {
    let mut rig = rig();
    rig.open_loaded();
    rig.open_themes();
    rig.ctrl('c');
    rig.pump_until(App::should_quit);
}

// -- previewing

#[test]
fn moving_the_cursor_applies_the_theme_to_every_mounted_component() {
    let mut rig = rig();
    rig.open_loaded();
    rig.app.show_toast("a toast").unwrap();
    rig.open_themes();
    assert_all_have(&rig.app, &theme("default"), 6);
    rig.press(Key::Char('j'));
    let seen = rig.pump_until(|app| app.theme_id() == "terminal");
    assert!(
        seen.contains(&Msg::PreviewTheme("terminal".into())),
        "{seen:?}"
    );
    assert_eq!(rig.app.theme(), &theme("terminal"));
    assert_all_have(&rig.app, &theme("terminal"), 6);
    let ids: Vec<Id> = mounted_themes(&rig.app)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    for id in [
        Id::Transcript,
        Id::BrowserHeader,
        Id::BrowserHelpLine,
        Id::EmptyState,
        Id::ThemeSelector,
        Id::Toast,
    ] {
        assert!(ids.contains(&id), "{id:?} in {ids:?}");
    }
    rig.move_to(Key::Down, "catppuccin");
    assert_all_have(&rig.app, &theme("catppuccin"), 6);
    rig.move_to(Key::Up, "terminal");
    assert_all_have(&rig.app, &theme("terminal"), 6);
}

#[test]
fn the_selector_screen_components_are_themed_too() {
    let mut rig = rig();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    let ids: Vec<Id> = mounted_themes(&rig.app)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    for id in [
        Id::SelectorFrame,
        Id::HarnessTabs,
        Id::ProjectList,
        Id::SessionList,
        Id::WorkingDir,
        Id::HelpLine,
    ] {
        assert!(ids.contains(&id), "{id:?} in {ids:?}");
    }
    assert_all_have(&rig.app, &theme("terminal"), 8);
}

#[test]
fn a_prompt_and_the_help_overlay_get_the_theme_too() {
    let mut rig = rig();
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.app.apply_theme_id("nord");
    assert_all_have(&rig.app, &theme("nord"), 7);
    assert!(
        mounted_themes(&rig.app)
            .iter()
            .any(|(id, _)| *id == Id::Prompt)
    );
    rig.press(Key::Esc);
    rig.pump_until(|app| !app.application().mounted(&Id::Prompt));
    rig.press(Key::Char('?'));
    rig.pump_until(|app| app.application().mounted(&Id::HelpOverlay));
    rig.app.apply_theme_id("solarized");
    assert_all_have(&rig.app, &theme("solarized"), 7);
    assert!(
        mounted_themes(&rig.app)
            .iter()
            .any(|(id, _)| *id == Id::HelpOverlay)
    );
}

#[test]
fn components_mounted_later_get_the_theme_that_was_kept() {
    let mut rig = rig();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    // The browser and a toast are mounted after the choice.
    rig.open_loaded();
    rig.app.show_toast("later").unwrap();
    assert_all_have(&rig.app, &theme("terminal"), 5);
    // And so is the selector over the browser.
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    assert_all_have(&rig.app, &theme("terminal"), 8);
}

#[test]
fn components_mounted_later_get_the_original_theme_after_esc() {
    let mut rig = rig();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.close_themes();
    rig.open_loaded();
    rig.app.show_toast("later").unwrap();
    assert_all_have(&rig.app, &theme("default"), 5);
}

#[test]
fn the_top_of_the_cursor_says_nothing() {
    let mut rig = rig();
    rig.open_themes();
    rig.press(Key::Up);
    rig.press(Key::Char('k'));
    let seen = rig.settle();
    assert!(
        !seen.iter().any(|m| matches!(m, Msg::PreviewTheme(_))),
        "{seen:?}"
    );
    assert_eq!(rig.app.theme_id(), "default");
}

#[test]
fn a_preview_keeps_the_terminal_adaptation_of_startup() {
    // Not truecolor: the colours are mapped to the 256-colour palette.
    let env = TerminalEnv::default();
    let registry = ThemeRegistry::builtin();
    let mut init = init_on("default");
    init.env = env.clone();
    init.theme = resolve_by_id(&registry, "default", &env, None).unwrap();
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    let expected = resolve_by_id(&registry, "terminal", &env, None).unwrap();
    assert_eq!(rig.app.theme(), &expected);
    rig.move_to(Key::Char('j'), "catppuccin");
    let expected = resolve_by_id(&registry, "catppuccin", &env, None).unwrap();
    assert_eq!(rig.app.theme(), &expected);
    assert_ne!(rig.app.theme(), &theme("catppuccin"), "not truecolor");
    assert_all_have(&rig.app, &expected, 6);
}

#[test]
fn a_preview_keeps_the_transparent_background_override() {
    let registry = ThemeRegistry::builtin();
    let mut init = init_on("default");
    init.config.transparent_background = Some(true);
    init.theme = resolve_by_id(&registry, "default", &truecolor(), Some(true)).unwrap();
    assert!(init.theme.transparent_background());
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    assert!(rig.app.theme().transparent_background());
    rig.move_to(Key::Char('j'), "catppuccin");
    assert!(rig.app.theme().transparent_background());
    assert_all_have(&rig.app, &rig.app.theme().clone(), 6);
}

#[test]
fn under_no_color_a_preview_switches_the_id_but_stays_colourless() {
    let registry = ThemeRegistry::builtin();
    let env = TerminalEnv {
        colorterm: None,
        no_color: true,
    };
    let colourless = resolve_by_id(&registry, "nord", &env, None).unwrap();
    assert_eq!(colourless.role(Role::Primary), Color::Reset);
    let mut init = init_on("nord");
    init.env = env;
    init.theme = colourless.clone();
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_themes();
    assert_eq!(rig.cursor().as_deref(), Some("nord"));
    rig.move_to(Key::Char('j'), "tokyo-night");
    assert_eq!(rig.app.theme(), &colourless);
    assert_all_have(&rig.app, &colourless, 6);
    rig.move_to(Key::Char('k'), "nord");
    // And Esc gives back the id it opened on.
    rig.move_to(Key::Char('j'), "tokyo-night");
    rig.close_themes();
    assert_eq!(rig.app.theme_id(), "nord");
}

/// The screen after previewing the theme is the screen of an app that started
/// with it: nothing keeps the colours of the old theme, in the overlay or
/// behind it.
#[test]
fn the_previewed_screen_is_the_screen_of_an_app_started_with_that_theme() {
    let mut previewed = rig();
    previewed.open_loaded();
    previewed.open_themes();
    for id in ["terminal", "catppuccin", "gruvbox"] {
        previewed.press(Key::Char('j'));
        previewed.pump_until(|app| app.theme_id() == id);
    }
    assert_eq!(previewed.cursor().as_deref(), Some("gruvbox"));
    let mut started = rig_on("gruvbox");
    started.open_loaded();
    started.open_themes();
    let a = screen_buffer(&mut previewed, 120, 40);
    let b = screen_buffer(&mut started, 120, 40);
    assert_eq!(buffer_to_styled(&a), buffer_to_styled(&b));
    // And it differs from the screen of the theme it started with.
    let mut first = rig();
    first.open_loaded();
    first.open_themes();
    let c = screen_buffer(&mut first, 120, 40);
    assert_ne!(buffer_to_styled(&a), buffer_to_styled(&c));
    // The transcript behind the overlay is gruvbox: its first row of text
    // uses a colour of that palette, which the default theme lacks.
    let gruvbox = theme("gruvbox");
    let default = theme("default");
    assert_eq!(a[(0, 0)].fg, gruvbox.role(Role::Border));
    assert_ne!(gruvbox.role(Role::Border), default.role(Role::Border));
    assert_eq!(c[(0, 0)].fg, default.role(Role::Border));
}

// -- Esc and Enter

#[test]
fn esc_restores_the_original_theme_exactly_and_returns_the_focus() {
    // A theme the registry would not give back by resolving its id.
    let custom = theme("gruvbox").with_role(Role::Primary, Color::Rgb(1, 2, 3));
    let mut init = init_on("gruvbox");
    init.theme = custom.clone();
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_loaded();
    rig.app.show_toast("t").unwrap();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "nord");
    rig.move_to(Key::Char('j'), "tokyo-night");
    assert_ne!(rig.app.theme(), &custom);
    rig.press(Key::Esc);
    let seen = rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    assert!(seen.contains(&Msg::CancelTheme), "{seen:?}");
    assert_eq!(rig.app.theme(), &custom);
    assert_eq!(rig.app.theme_id(), "gruvbox");
    assert_all_have(&rig.app, &custom, 5);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.config().theme, "default", "nothing was chosen");
}

#[test]
fn esc_without_moving_changes_nothing() {
    let mut rig = rig_on("nord");
    rig.open_themes();
    rig.close_themes();
    assert_eq!(rig.app.theme(), &theme("nord"));
    assert_eq!(rig.app.theme_id(), "nord");
}

#[test]
fn esc_saves_nothing() {
    let (mut rig, dir) = rig_saving(None);
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.close_themes();
    assert_eq!(config_text(&dir), None);
}

#[test]
fn enter_keeps_the_theme_closes_the_selector_and_returns_the_focus() {
    let (mut rig, _dir) = rig_saving(None);
    rig.open_loaded();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.move_to(Key::Char('j'), "catppuccin");
    rig.press(Key::Enter);
    let seen = rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    assert!(
        seen.contains(&Msg::SelectTheme("catppuccin".into())),
        "{seen:?}"
    );
    assert_eq!(rig.app.theme(), &theme("catppuccin"));
    assert_eq!(rig.app.theme_id(), "catppuccin");
    assert_eq!(rig.app.config().theme, "catppuccin");
    assert_all_have(&rig.app, &theme("catppuccin"), 5);
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.toast_text(), None);
    // The next open starts on it.
    rig.open_themes();
    assert_eq!(rig.cursor().as_deref(), Some("catppuccin"));
    rig.close_themes();
    assert_eq!(rig.app.theme_id(), "catppuccin");
}

#[test]
fn enter_without_moving_saves_the_active_theme() {
    let (mut rig, dir) = rig_saving(None);
    rig.open_themes();
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    assert_eq!(config_text(&dir).as_deref(), Some("theme = \"default\"\n"));
}

// -- saving

#[test]
fn enter_creates_a_missing_config_with_exactly_the_theme() {
    let (mut rig, dir) = rig_saving(None);
    assert_eq!(config_text(&dir), None);
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    assert_eq!(config_text(&dir).as_deref(), Some("theme = \"terminal\"\n"));
}

#[test]
fn enter_changes_only_the_theme_in_an_existing_config() {
    let before = "# my settings\nmouse = false # no mice\n\n# the look\ntheme = \"default\"  # for now\nutc = true\n\n[keys.global]\n# quit\nquit = \"x\"\n";
    let (mut rig, dir) = rig_saving(Some(before));
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.move_to(Key::Char('j'), "catppuccin");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    let after = config_text(&dir).unwrap();
    assert_eq!(after, before.replace("\"default\"", "\"catppuccin\""));
    assert_eq!(rig.app.toast_text(), None);
}

#[test]
fn a_config_that_is_not_toml_is_left_alone_and_a_toast_says_so() {
    let before = "theme = \"default\"\nthis is [ not toml\n";
    let (mut rig, dir) = rig_saving(Some(before));
    rig.open_loaded();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    assert_eq!(config_text(&dir).as_deref(), Some(before));
    assert_eq!(
        rig.app.toast_text(),
        Some("Could not save theme: config.toml is invalid")
    );
    // The theme stays applied for this run, and the focus is back.
    assert_eq!(rig.app.theme(), &theme("terminal"));
    assert_eq!(rig.focus(), Some(Id::Transcript));
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    assert!(
        rows.iter()
            .any(|r| r.contains("Could not save theme: config.toml is invalid")),
        "the toast is drawn"
    );
}

#[test]
fn a_write_error_shows_a_toast_with_the_error_and_keeps_the_theme() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    std::fs::write(&file, "x").unwrap();
    let mut init = init_on("default");
    init.config_path = Some(file.join("config.toml"));
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    let toast = rig.app.toast_text().unwrap();
    assert!(toast.starts_with("Could not save theme: "), "{toast}");
    assert!(toast.len() > "Could not save theme: ".len(), "{toast}");
    assert_ne!(toast, "Could not save theme: config.toml is invalid");
    assert_eq!(rig.app.theme(), &theme("terminal"));
}

#[test]
fn without_a_config_location_a_toast_says_it_was_not_saved() {
    let mut rig = rig();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.application().mounted(&Id::ThemeSelector));
    let toast = rig.app.toast_text().unwrap();
    assert!(toast.starts_with("Could not save theme: "), "{toast}");
    assert_eq!(rig.app.theme(), &theme("terminal"));
}

// -- messages that arrive late

#[test]
fn the_messages_of_a_closed_selector_are_ignored() {
    let (mut rig, dir) = rig_saving(None);
    rig.app.update(Msg::PreviewTheme("nord".into()));
    assert_eq!(rig.app.theme_id(), "default");
    rig.app.update(Msg::SelectTheme("nord".into()));
    assert_eq!(rig.app.theme_id(), "default");
    assert_eq!(rig.app.config().theme, "default");
    assert_eq!(config_text(&dir), None);
    rig.app.update(Msg::CancelTheme);
    assert_eq!(rig.app.theme_id(), "default");
    assert_eq!(rig.focus(), Some(Id::ProjectList));
}

#[test]
fn a_preview_that_arrives_after_esc_in_the_same_batch_does_not_stick() {
    let mut rig = rig();
    rig.open_themes();
    rig.app.update(Msg::CancelTheme);
    rig.app.update(Msg::PreviewTheme("nord".into()));
    assert!(!rig.themes_open());
    assert_eq!(rig.app.theme_id(), "default");
    assert_eq!(rig.app.theme(), &theme("default"));
}

#[test]
fn a_theme_that_cannot_be_resolved_is_not_applied() {
    let mut rig = rig();
    rig.open_themes();
    rig.app.update(Msg::PreviewTheme("no-such-theme".into()));
    assert_eq!(rig.app.theme_id(), "default");
    assert_eq!(rig.app.theme(), &theme("default"));
}

#[test]
fn opening_another_session_drops_the_selector_and_gives_back_the_theme() {
    let mut rig = rig();
    rig.open_themes();
    rig.move_to(Key::Char('j'), "terminal");
    rig.app.update(Msg::OpenSession(CLAUDE, id(BRIO_SESSION)));
    assert!(!rig.themes_open());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert_eq!(rig.app.theme_id(), "default");
    assert_all_have(&rig.app, &theme("default"), 4);
}

// -- user themes

#[test]
fn an_invalid_user_theme_is_skipped_by_the_cursor_and_never_applied() {
    let registry = registry_with(&[("a-user", Some("A User")), ("my-theme", None)]);
    let mut rig = rig_with_registry(registry, 120, 40);
    rig.open_themes();
    for id in [
        "terminal",
        "catppuccin",
        "gruvbox",
        "nord",
        "tokyo-night",
        "solarized",
    ] {
        rig.move_to(Key::Char('j'), id);
    }
    rig.move_to(Key::Char('j'), "a-user");
    assert_eq!(rig.app.theme().name(), "A User");
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(
        rig.cursor().as_deref(),
        Some("a-user"),
        "my-theme is invalid"
    );
    assert_eq!(rig.app.theme_id(), "a-user");
}

#[test]
fn an_active_user_theme_starts_the_cursor_on_it() {
    let registry = registry_with(&[("a-user", Some("A User"))]);
    let mut init = init_on("default");
    init.theme = resolve_by_id(&registry, "a-user", &truecolor(), None).unwrap();
    init.theme_id = "a-user".to_owned();
    init.registry = registry;
    let mut rig = build_rig_from(init, 120, 40, false);
    rig.open_themes();
    assert_eq!(rig.cursor().as_deref(), Some("a-user"));
}

// -- screens

#[test]
fn the_themes_over_the_browser_120x40_default_text() {
    let mut rig = open_at(120, 40);
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.contains("╭─ Themes ─"));
    assert!(text.contains("│  > Default  "), "the cursor is on Default");
    insta::assert_snapshot!(text);
}

#[test]
fn the_themes_over_the_browser_120x40_default_styled() {
    let mut rig = open_at(120, 40);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 120, 40)));
}

#[test]
fn the_themes_after_three_j_120x40_gruvbox_styled() {
    let mut rig = open_at(120, 40);
    for id in ["terminal", "catppuccin", "gruvbox"] {
        rig.press(Key::Char('j'));
        rig.pump_until(|app| app.theme_id() == id);
    }
    let buffer = screen_buffer(&mut rig, 120, 40);
    let rows = rows_of(&buffer);
    assert!(rows.iter().any(|r| r.contains("│  > Gruvbox  ")));
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

#[test]
fn the_themes_over_the_selector_80x24_text() {
    let mut rig = rig();
    rig.open_themes();
    let text = screen_text(&mut rig, 80, 24);
    assert!(text.contains("[1] Claude"), "the selector is around it");
    assert!(text.contains("╭─ Themes ─"));
    insta::assert_snapshot!(text);
}

fn user_themes_rig() -> Rig {
    let registry = registry_with(&[("aurora", Some("Aurora")), ("my-theme", None)]);
    let mut rig = rig_with_registry(registry, 120, 40);
    rig.open_loaded();
    rig.open_themes();
    rig
}

#[test]
fn the_themes_with_two_user_themes_one_invalid_120x40_text() {
    let mut rig = user_themes_rig();
    let text = screen_text(&mut rig, 120, 40);
    assert!(text.contains("│    Aurora  "));
    assert!(text.contains("│    my-theme (invalid)  "));
    insta::assert_snapshot!(text);
}

#[test]
fn the_themes_with_two_user_themes_one_invalid_120x40_styled() {
    let mut rig = user_themes_rig();
    let buffer = screen_buffer(&mut rig, 120, 40);
    // `(invalid)` is muted, like the rows the cursor is not on.
    let rows = rows_of(&buffer);
    let y = rows.iter().position(|r| r.contains("my-theme")).unwrap();
    let x = rows[y].find("my-theme").unwrap();
    let muted = theme("default").role(Role::MutedForeground);
    let cell = &buffer[(u16::try_from(x).unwrap(), u16::try_from(y).unwrap())];
    assert_eq!(cell.fg, muted);
    insta::assert_snapshot!(buffer_to_styled(&buffer));
}

/// Nine user themes make sixteen rows, which 60x20 does not fit.
fn sixteen_themes() -> ThemeRegistry {
    let ids: Vec<String> = (1..=9).map(|i| format!("user-{i:02}")).collect();
    let names: Vec<String> = (1..=9).map(|i| format!("User {i:02}")).collect();
    let user: Vec<(&str, Option<&str>)> = ids
        .iter()
        .zip(&names)
        .map(|(id, name)| (id.as_str(), Some(name.as_str())))
        .collect();
    registry_with(&user)
}

#[test]
fn sixteen_themes_at_60x20_scroll_with_the_cursor_near_the_end() {
    let mut rig = rig_with_registry(sixteen_themes(), 60, 20);
    rig.open_themes();
    let ids: Vec<&str> = [
        "terminal",
        "catppuccin",
        "gruvbox",
        "nord",
        "tokyo-night",
        "solarized",
        "user-01",
        "user-02",
        "user-03",
        "user-04",
        "user-05",
        "user-06",
        "user-07",
        "user-08",
    ]
    .to_vec();
    for id in ids {
        rig.move_to(Key::Char('j'), id);
    }
    assert_eq!(rig.cursor().as_deref(), Some("user-08"));
    let text = screen_text(&mut rig, 60, 20);
    let rows: Vec<&str> = text.lines().collect();
    assert!(rows.iter().any(|r| r.contains("│  > User 08  ")));
    assert!(!text.contains("Default"), "scrolled past the top");
    assert!(!text.contains("Terminal"));
    assert!(text.contains("Catppuccin"));
    assert!(
        !text.contains("User 09"),
        "the last theme is below the list"
    );
    assert_eq!(rows.iter().filter(|r| r.contains("User 0")).count(), 8);
    insta::assert_snapshot!(text);
}

#[test]
fn the_list_follows_the_cursor_when_the_terminal_shrinks() {
    let mut rig = rig_with_registry(sixteen_themes(), 120, 40);
    rig.open_themes();
    for _ in 0..15 {
        rig.press(Key::Char('j'));
    }
    rig.pump_until(|app| app.theme_id() == "user-09");
    let big = screen_text(&mut rig, 120, 40);
    assert!(big.contains("Default"), "all sixteen fit in 21 rows");
    let small = screen_text(&mut rig, 60, 20);
    assert!(!small.contains("Default"), "{small}");
    assert!(small.contains("│  > User 09  "), "{small}");
    let big = screen_text(&mut rig, 120, 40);
    assert!(big.contains("│  > User 09  "));
}

#[test]
fn twelve_themes_fit_exactly_at_60x20() {
    // 12 rows + 5 rows of chrome = 17, and the overlay may use 20 - 2.
    let user: Vec<(&str, Option<&str>)> = ["u1", "u2", "u3", "u4", "u5"]
        .into_iter()
        .map(|id| (id, Some("User 5")))
        .collect();
    let mut rig = rig_with_registry(registry_with(&user), 60, 20);
    rig.open_themes();
    for _ in 0..11 {
        rig.press(Key::Char('j'));
    }
    rig.pump_until(|app| app.theme_id() == "u5");
    let text = screen_text(&mut rig, 60, 20);
    assert!(text.contains("Default"), "nothing scrolled");
    assert!(text.contains("│  > User 5  "));
    let rows: Vec<&str> = text.lines().collect();
    let top = rows.iter().position(|r| r.contains("╭─ Themes")).unwrap();
    let border = format!("╰{}╯", "─".repeat(39));
    let bottom = rows.iter().position(|r| r.contains(&border)).unwrap();
    assert_eq!(bottom - top + 1, 17);
}

#[test]
fn the_overlay_is_centred_and_hidden_on_a_screen_that_is_too_small() {
    let mut rig = open_at(120, 40);
    let rows = rows_of(&screen_buffer(&mut rig, 120, 40));
    let y = rows.iter().position(|r| r.contains("╭─ Themes")).unwrap();
    let x = rows[y].chars().position(|c| c == '╭').unwrap();
    // 41 columns wide, 12 rows tall.
    assert_eq!(x, (120 - 41) / 2);
    assert_eq!(y, (40 - 12) / 2);
    let small = screen_text(&mut rig, 50, 15);
    assert!(small.contains("Terminal too small"));
    assert!(!small.contains("Themes"));
    assert!(rig.themes_open());
    let big = screen_text(&mut rig, 120, 40);
    assert!(big.contains("╭─ Themes"));
}

#[test]
fn a_toast_is_drawn_over_the_selector() {
    let mut rig = open_at(80, 24);
    rig.app.show_toast("copied").unwrap();
    let rows = rows_of(&screen_buffer(&mut rig, 80, 24));
    assert!(rows.iter().any(|r| r.contains("│ copied │")), "{rows:?}");
    assert!(rows.iter().any(|r| r.contains("╭─ Themes")));
}
