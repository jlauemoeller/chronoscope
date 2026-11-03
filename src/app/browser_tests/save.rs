//! Saving a section and exporting the session (SPEC §6.4, §9.5) through the
//! app: the keys, the path and overwrite prompts, the files, the toasts and
//! the screens.
//!
//! The project is called `brio` here (the display name of a project without
//! an alias is its directory's name), so the default names read as the spec
//! gives them. Files go to a temporary directory the app is told to resolve
//! relative paths against; nothing is written to the working directory.

use tuirealm::state::{State, StateValue};

use super::*;

const SAVE_DEFAULT: &str = "./brio-85a86657-shell.txt";
const EXPORT_DEFAULT: &str = "./brio-20261021-0835-85a86657.md";

fn prompt_text(app: &App) -> String {
    match app.application().state(&Id::Prompt).unwrap() {
        State::Single(StateValue::String(text)) => text,
        other => panic!("{other:?}"),
    }
}

/// A rig of `width` x `height` with the BRIO session open, the project
/// called `brio`, and the app resolving relative paths in a new temporary
/// directory (with a home directory `home` in it).
fn save_rig(width: u16, height: u16) -> (Rig, tempfile::TempDir) {
    save_rig_opening(width, height, Rig::open_loaded)
}

/// Like [`save_rig`], with the session opened by `open`.
fn save_rig_opening(
    width: u16,
    height: u16,
    open: impl FnOnce(&mut Rig),
) -> (Rig, tempfile::TempDir) {
    let mut rig = rig_with(theme("default"), width, height);
    rig.app.update(fixtures::message(index_named("brio")));
    open(&mut rig);
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("home")).unwrap();
    rig.app.set_save_dir(dir.path().to_path_buf());
    rig.app.set_home_dir(Some(dir.path().join("home")));
    (rig, dir)
}

impl Rig {
    /// Moves the selection down to the first Shell call of the fixture.
    fn select_shell_call(&self) {
        for _ in 0..3 {
            self.press(Key::Char('j'));
        }
    }

    /// Presses the key and waits for the path prompt.
    fn ask(&mut self, code: Key, modifiers: KeyModifiers) -> String {
        self.key(code, modifiers);
        self.pump_until(App::write_prompt_open);
        prompt_text(&self.app)
    }

    /// Presses `Esc` and waits until the prompt is gone.
    fn dismiss(&mut self) {
        self.press(Key::Esc);
        self.pump_until(|app| !app.write_prompt_open());
    }

    /// Types `text` into the prompt.
    fn type_in_prompt(&self, text: &str) {
        text.chars().for_each(|c| self.press(Key::Char(c)));
    }

    fn prompt_mounted(&self) -> bool {
        self.app.application().mounted(&Id::Prompt)
    }

    fn shown_toast(&self) -> Option<String> {
        self.app.toast_text().map(str::to_owned)
    }

    /// Answers the path prompt with `path`, as `Enter` would.
    fn submit(&mut self, path: &str) {
        self.app.update(Msg::PromptSubmit(path.to_owned()));
    }
}

/// The turns of the fixture session, every tool call finished after 1.2 s.
fn timed_turns() -> Vec<Arc<Turn>> {
    turns()
        .into_iter()
        .map(|mut turn| {
            if let Turn::Assistant(assistant) = Arc::make_mut(&mut turn) {
                for item in &mut assistant.items {
                    if let crate::model::Item::ToolCall(call) = item {
                        call.duration = Some(Duration::from_millis(1_200));
                    }
                }
            }
            turn
        })
        .collect()
}

impl Rig {
    /// Opens the BRIO session with its tool calls timed.
    fn open_timed(&mut self) {
        self.harness.script(
            BRIO_SESSION,
            vec![
                ModelUpdate::Appended(timed_turns()),
                ModelUpdate::Progress(1.0),
                ModelUpdate::ParseWarnings(0),
                ModelUpdate::Loaded,
            ],
        );
        self.open(BRIO_SESSION);
    }
}

fn path_str(dir: &tempfile::TempDir, name: &str) -> String {
    dir.path().join(name).display().to_string()
}

// -- the keys and the default names

#[test]
fn w_ctrl_s_and_capital_w_ask_for_a_path_with_the_default_save_name() {
    let (mut rig, _dir) = save_rig(120, 40);
    rig.select_shell_call();
    for (code, modifiers) in [
        (Key::Char('w'), KeyModifiers::NONE),
        (Key::Char('s'), KeyModifiers::CONTROL),
        (Key::Char('W'), KeyModifiers::SHIFT),
    ] {
        assert_eq!(rig.ask(code, modifiers), SAVE_DEFAULT, "{code:?}");
        assert_eq!(rig.focus(), Some(Id::Prompt));
        rig.dismiss();
        assert_eq!(rig.focus(), Some(Id::Transcript));
    }
}

#[test]
fn e_ctrl_e_and_capital_e_ask_for_a_path_with_the_default_export_name() {
    let (mut rig, _dir) = save_rig(120, 40);
    for (code, modifiers) in [
        (Key::Char('e'), KeyModifiers::NONE),
        (Key::Char('e'), KeyModifiers::CONTROL),
        (Key::Char('E'), KeyModifiers::SHIFT),
    ] {
        assert_eq!(rig.ask(code, modifiers), EXPORT_DEFAULT, "{code:?}");
        assert_eq!(rig.focus(), Some(Id::Prompt));
        rig.dismiss();
        assert_eq!(rig.focus(), Some(Id::Transcript));
    }
}

#[test]
fn the_save_name_has_the_kind_of_the_selected_section() {
    let (mut rig, _dir) = save_rig(120, 40);
    // The first section of the fixture is the prompt.
    assert_eq!(
        rig.ask(Key::Char('w'), KeyModifiers::NONE),
        "./brio-85a86657-user.txt"
    );
    rig.dismiss();
    // The text of the assistant is the second.
    rig.press(Key::Char('j'));
    assert_eq!(
        rig.ask(Key::Char('w'), KeyModifiers::NONE),
        "./brio-85a86657-assistant.txt"
    );
}

#[test]
fn the_default_name_sanitises_the_project_and_uses_the_first_eight_characters_of_the_id() {
    let mut rig = rig_with(theme("default"), 120, 40);
    rig.app
        .update(fixtures::message(index_named("My Project/x")));
    rig.open_loaded();
    assert_eq!(
        rig.ask(Key::Char('w'), KeyModifiers::NONE),
        "./My-Project-x-85a86657-user.txt"
    );
}

#[test]
fn the_prompt_has_the_cursor_at_the_end_of_the_default_path() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.press(Key::Char('2'));
    let typed = format!("{SAVE_DEFAULT}2");
    rig.pump_until(|app| prompt_text(app) == typed);
    assert!(!dir.path().join("brio-85a86657-shell.txt2").exists());
}

#[test]
fn every_printable_key_goes_into_the_path() {
    let (mut rig, _dir) = save_rig(120, 40);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    // `q` quits and `j`, `t` and `1` have meanings in the browser.
    rig.type_in_prompt("qjt1");
    rig.pump_until(|app| prompt_text(app) == "./brio-85a86657-user.txtqjt1");
    rig.settle();
    assert!(!rig.app.should_quit());
    assert_eq!(rig.focus(), Some(Id::Prompt));
}

#[test]
fn ctrl_c_in_the_prompt_still_quits() {
    let (mut rig, _dir) = save_rig(120, 40);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.key(Key::Char('c'), KeyModifiers::CONTROL);
    rig.pump_until(App::should_quit);
}

#[test]
fn the_save_keys_do_nothing_before_a_session_is_open() {
    let mut rig = rig();
    rig.app.update(Msg::SaveSection {
        text: "x".to_owned(),
        kind: "user",
    });
    rig.app.update(Msg::Export {
        full: false,
        document: "x".to_owned(),
    });
    assert!(!rig.prompt_mounted());
    assert!(!rig.app.write_prompt_open());
}

// -- writing

#[test]
fn a_path_that_does_not_exist_is_written_at_once() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit(SAVE_DEFAULT);
    assert!(!rig.prompt_mounted());
    assert!(!rig.app.write_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    let written = std::fs::read_to_string(dir.path().join("brio-85a86657-shell.txt")).unwrap();
    assert_eq!(
        written, "Find the BRIO on USB and check available tools",
        "the section as displayed: folded, its description"
    );
    assert_eq!(
        rig.shown_toast().as_deref(),
        Some("Saved to ./brio-85a86657-shell.txt")
    );
}

#[test]
fn capital_w_saves_the_full_content_of_the_section() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    rig.ask(Key::Char('W'), KeyModifiers::SHIFT);
    rig.submit("./full.txt");
    let written = std::fs::read_to_string(dir.path().join("full.txt")).unwrap();
    assert_eq!(
        written,
        "Find the BRIO on USB and check available tools\n\nCommand:\n\nlsusb | grep -i brio\n\nResult:\n\nBus 001 Device 004: ID 046d:085e Logitech, Inc. BRIO"
    );
    assert_eq!(rig.shown_toast().as_deref(), Some("Saved to ./full.txt"));
}

#[test]
fn w_saves_the_duration_with_the_description_and_capital_w_with_the_full_content() {
    let (mut rig, dir) = save_rig_opening(120, 40, Rig::open_timed);
    rig.select_shell_call();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit("./shown.txt");
    let shown = std::fs::read_to_string(dir.path().join("shown.txt")).unwrap();
    assert_eq!(
        shown,
        "Find the BRIO on USB and check available tools · 1.2s"
    );
    rig.ask(Key::Char('W'), KeyModifiers::SHIFT);
    rig.submit("./full.txt");
    let full = std::fs::read_to_string(dir.path().join("full.txt")).unwrap();
    assert!(
        full.starts_with("Find the BRIO on USB and check available tools · 1.2s\n\nCommand:\n"),
        "{full}"
    );
}

#[test]
fn enter_in_the_prompt_writes_the_path_that_was_typed() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    for _ in 0..SAVE_DEFAULT.len() {
        rig.press(Key::Backspace);
    }
    rig.type_in_prompt("typed.txt");
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.write_prompt_open());
    assert!(dir.path().join("typed.txt").is_file());
    assert_eq!(rig.shown_toast().as_deref(), Some("Saved to typed.txt"));
}

#[test]
fn esc_in_the_path_prompt_writes_nothing() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.dismiss();
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.prompt_mounted());
    assert_eq!(rig.shown_toast(), None);
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "home only"
    );
}

#[test]
fn an_empty_path_writes_nothing() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit("  ");
    assert!(!rig.prompt_mounted());
    assert_eq!(rig.shown_toast(), None);
    assert_eq!(
        std::fs::read_dir(dir.path()).unwrap().count(),
        1,
        "home only"
    );
}

#[test]
fn a_path_may_be_absolute() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    let path = path_str(&dir, "absolute.txt");
    rig.submit(&path);
    assert!(dir.path().join("absolute.txt").is_file());
    // The toast cuts a long path at its start, and the temporary directory can
    // be long enough for that, as it is on macOS.
    let toast = rig.shown_toast().expect("a toast");
    assert!(toast.starts_with("Saved to "), "{toast}");
    assert!(toast.ends_with("/absolute.txt"), "{toast}");
}

#[test]
fn a_tilde_is_the_home_directory() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit("~/in-home.txt");
    assert!(dir.path().join("home/in-home.txt").is_file());
    assert_eq!(rig.shown_toast().as_deref(), Some("Saved to ~/in-home.txt"));
}

#[test]
fn a_missing_parent_directory_is_a_failure_and_none_is_created() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit("no/such/dir/out.txt");
    let toast = rig.shown_toast().unwrap();
    assert!(toast.starts_with("Save failed: "), "{toast}");
    assert!(toast.contains("No such file"), "{toast}");
    assert!(!dir.path().join("no").exists());
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn an_export_that_cannot_be_written_says_export_failed() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    rig.submit("no/such/dir/out.md");
    let toast = rig.shown_toast().unwrap();
    assert!(toast.starts_with("Export failed: "), "{toast}");
    assert!(!dir.path().join("no").exists());
}

#[test]
fn a_directory_is_not_asked_about_and_fails_to_be_written() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit("home");
    assert!(!rig.prompt_mounted());
    assert!(rig.shown_toast().unwrap().starts_with("Save failed: "));
    assert!(dir.path().join("home").is_dir());
}

// -- overwriting

/// Saves the selected Shell call to the default path, which exists with
/// `old contents`: the overwrite prompt is open.
fn existing(rig: &mut Rig, dir: &tempfile::TempDir) -> std::path::PathBuf {
    let path = dir.path().join("brio-85a86657-shell.txt");
    std::fs::write(&path, "old contents").unwrap();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit(SAVE_DEFAULT);
    path
}

#[test]
fn a_path_that_exists_asks_before_overwriting_and_writes_nothing_yet() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    let path = existing(&mut rig, &dir);
    assert!(rig.prompt_mounted());
    assert!(rig.app.write_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old contents");
    assert_eq!(rig.shown_toast(), None);
}

#[test]
fn y_overwrites() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    let path = existing(&mut rig, &dir);
    rig.press(Key::Char('y'));
    rig.pump_until(|app| !app.write_prompt_open());
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "Find the BRIO on USB and check available tools"
    );
    assert_eq!(
        rig.shown_toast().as_deref(),
        Some("Saved to ./brio-85a86657-shell.txt")
    );
    assert!(!rig.prompt_mounted());
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn capital_y_overwrites_too() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    let path = existing(&mut rig, &dir);
    rig.key(Key::Char('Y'), KeyModifiers::SHIFT);
    rig.pump_until(|app| !app.write_prompt_open());
    assert_ne!(std::fs::read_to_string(&path).unwrap(), "old contents");
}

#[test]
fn n_leaves_the_file_untouched() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    let path = existing(&mut rig, &dir);
    rig.press(Key::Char('n'));
    rig.pump_until(|app| !app.write_prompt_open());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old contents");
    assert_eq!(rig.shown_toast(), None);
    assert!(!rig.prompt_mounted());
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn esc_leaves_the_file_untouched() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    let path = existing(&mut rig, &dir);
    rig.dismiss();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old contents");
    assert_eq!(rig.shown_toast(), None);
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn any_other_key_than_y_is_a_no() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    let path = existing(&mut rig, &dir);
    rig.press(Key::Enter);
    rig.pump_until(|app| !app.write_prompt_open());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old contents");
}

#[test]
fn an_existing_export_asks_too() {
    let (mut rig, dir) = save_rig(120, 40);
    let path = dir.path().join("brio-20261021-0835-85a86657.md");
    std::fs::write(&path, "old").unwrap();
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    rig.submit(EXPORT_DEFAULT);
    assert!(rig.app.write_prompt_open());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
    rig.press(Key::Char('y'));
    rig.pump_until(|app| !app.write_prompt_open());
    assert!(
        std::fs::read_to_string(&path)
            .unwrap()
            .starts_with("# Investigate the camera profiles\n")
    );
    assert_eq!(
        rig.shown_toast().as_deref(),
        Some("Exported to ./brio-20261021-0835-85a86657.md")
    );
}

// -- exporting

#[test]
fn e_exports_the_session_as_currently_folded() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    rig.submit(EXPORT_DEFAULT);
    let document =
        std::fs::read_to_string(dir.path().join("brio-20261021-0835-85a86657.md")).unwrap();
    assert!(
        document.starts_with(
            "# Investigate the camera profiles\n\n- Project: brio\n- Session: 85a86657-11a7-4318-a21a-0f4476babd5c\n- Working directory: /home/user/projects/brio\n- Started: 2026-10-21 08:35:15Z\n- Harness: Claude\n\n## User — "
        ),
        "{document}"
    );
    // Folded: the tool call is its summary line; the system item (shown by
    // default, folded) is its summary line too.
    assert!(
        document.contains("> Find the BRIO on USB and check available tools\n"),
        "{document}"
    );
    assert!(!document.contains("<details>"));
    assert_eq!(
        rig.shown_toast().as_deref(),
        Some("Exported to ./brio-20261021-0835-85a86657.md")
    );
}

#[test]
fn e_follows_the_folds_the_user_made() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.select_shell_call();
    // Unfold the tool call.
    rig.press(Key::Enter);
    rig.settle();
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    rig.submit("folded.md");
    let document = std::fs::read_to_string(dir.path().join("folded.md")).unwrap();
    assert!(
        document.contains("**Find the BRIO on USB and check available tools**\n\n**Command:**\n\n```sh\nlsusb | grep -i brio\n```"),
        "{document}"
    );
}

#[test]
fn capital_e_exports_the_full_session() {
    let (mut rig, dir) = save_rig(120, 40);
    rig.ask(Key::Char('E'), KeyModifiers::SHIFT);
    rig.submit("full.md");
    let document = std::fs::read_to_string(dir.path().join("full.md")).unwrap();
    assert!(
        document.contains("<details><summary>Find the BRIO on USB and check available tools</summary>\n\n```sh\nlsusb | grep -i brio\n```\n\n```\nBus 001 Device 004"),
        "{document}"
    );
    assert!(document.contains("</details>"));
    assert_eq!(rig.shown_toast().as_deref(), Some("Exported to full.md"));
}

#[test]
fn the_full_export_has_the_system_content_the_toggle_hides() {
    let (mut rig, dir) = save_rig(120, 40);
    // `s` hides the system sections of the transcript.
    rig.press(Key::Char('s'));
    rig.settle();
    rig.ask(Key::Char('E'), KeyModifiers::SHIFT);
    rig.submit("full.md");
    let full = std::fs::read_to_string(dir.path().join("full.md")).unwrap();
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    rig.submit("folded.md");
    let folded = std::fs::read_to_string(dir.path().join("folded.md")).unwrap();
    assert!(full.contains("Use the vendor GUIDs."), "{full}");
    assert!(!folded.contains("vendor GUIDs"), "{folded}");
    assert!(!folded.contains("notes.md"), "{folded}");
}

// -- harness data

fn protected(rig: &Rig, dir: &tempfile::TempDir) -> std::path::PathBuf {
    let data = dir.path().join("data");
    std::fs::create_dir_all(data.join("projects")).unwrap();
    *rig.harness.watch.lock().unwrap() = vec![data.clone()];
    data
}

fn refused(rig: &mut Rig, path: &str) {
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit(path);
    assert_eq!(
        rig.shown_toast().as_deref(),
        Some("Refusing to write into harness data"),
        "{path}"
    );
    assert!(!rig.prompt_mounted(), "{path}");
    assert_eq!(rig.focus(), Some(Id::Transcript));
}

#[test]
fn writing_into_the_data_of_a_harness_is_refused() {
    let (mut rig, dir) = save_rig(120, 40);
    let data = protected(&rig, &dir);
    refused(&mut rig, &path_str(&dir, "data/out.txt"));
    assert!(!data.join("out.txt").exists());
    refused(&mut rig, &path_str(&dir, "data/projects/deeper/out.txt"));
    assert!(!data.join("projects/deeper").exists());
}

#[test]
fn the_claude_data_directory_is_refused() {
    // The real Claude harness, with a home directory of its own: its data
    // directory is `<home>/.claude/projects`.
    let (mut rig, dir) = save_rig(120, 40);
    let home = dir.path().join("home");
    let data = crate::harness::claude::default_data_dir(&home);
    std::fs::create_dir_all(data.join("-home-user-brio")).unwrap();
    let claude =
        crate::harness::claude::ClaudeHarness::new(&crate::config::Config::default(), &home);
    rig.app
        .set_harnesses([std::sync::Arc::new(claude) as std::sync::Arc<dyn Harness>]);
    refused(&mut rig, "~/.claude/projects/out.txt");
    refused(&mut rig, "~/.claude/projects/-home-user-brio/session.jsonl");
    assert!(!data.join("out.txt").exists());
    assert!(!data.join("-home-user-brio/session.jsonl").exists());
    // The rest of `.claude` is not harness data.
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit("~/.claude/notes.txt");
    assert_eq!(
        rig.shown_toast().as_deref(),
        Some("Saved to ~/.claude/notes.txt")
    );
}

#[test]
fn an_export_into_harness_data_is_refused_too() {
    let (mut rig, dir) = save_rig(120, 40);
    let data = protected(&rig, &dir);
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    rig.submit(&path_str(&dir, "data/out.md"));
    assert_eq!(
        rig.shown_toast().as_deref(),
        Some("Refusing to write into harness data")
    );
    assert!(!data.join("out.md").exists());
}

#[test]
fn a_relative_path_into_harness_data_is_refused() {
    let (mut rig, dir) = save_rig(120, 40);
    let data = protected(&rig, &dir);
    refused(&mut rig, "data/out.txt");
    // And one that starts in it.
    rig.app.set_save_dir(data.clone());
    refused(&mut rig, "./out.txt");
    assert!(!data.join("out.txt").exists());
}

#[test]
fn a_path_that_climbs_back_into_harness_data_is_refused() {
    let (mut rig, dir) = save_rig(120, 40);
    let data = protected(&rig, &dir);
    refused(&mut rig, "home/../data/out.txt");
    refused(&mut rig, "data/projects/../../data/out.txt");
    refused(&mut rig, "data/missing/../out.txt");
    assert!(!data.join("out.txt").exists());
}

#[cfg(unix)]
#[test]
fn a_symlink_into_harness_data_is_refused() {
    let (mut rig, dir) = save_rig(120, 40);
    let data = protected(&rig, &dir);
    std::os::unix::fs::symlink(&data, dir.path().join("link")).unwrap();
    refused(&mut rig, "link/out.txt");
    assert!(!data.join("out.txt").exists());
}

#[cfg(unix)]
#[test]
fn a_dangling_symlink_into_harness_data_is_refused_without_asking() {
    let (mut rig, dir) = save_rig(120, 40);
    let data = protected(&rig, &dir);
    // The link exists, what it points to does not: a write through it would
    // create the file in the data directory.
    std::os::unix::fs::symlink(data.join("new.txt"), dir.path().join("dangling.txt")).unwrap();
    std::os::unix::fs::symlink("data/projects/rel.txt", dir.path().join("relative.txt")).unwrap();
    for (link, target) in [
        ("dangling.txt", "new.txt"),
        ("relative.txt", "projects/rel.txt"),
    ] {
        refused(&mut rig, link);
        assert!(!data.join(target).exists(), "{target}");
    }
    assert!(!rig.app.write_prompt_open());
}

#[test]
fn an_existing_file_in_harness_data_is_refused_without_asking() {
    let (mut rig, dir) = save_rig(120, 40);
    let data = protected(&rig, &dir);
    std::fs::write(data.join("session.jsonl"), "keep").unwrap();
    refused(&mut rig, "data/session.jsonl");
    assert_eq!(
        std::fs::read_to_string(data.join("session.jsonl")).unwrap(),
        "keep"
    );
}

#[test]
fn a_directory_next_to_harness_data_is_not_refused() {
    let (mut rig, dir) = save_rig(120, 40);
    protected(&rig, &dir);
    std::fs::create_dir(dir.path().join("data2")).unwrap();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit("data2/out.txt");
    assert!(dir.path().join("data2/out.txt").is_file());
    assert_eq!(rig.shown_toast().as_deref(), Some("Saved to data2/out.txt"));
}

// -- the screens

fn screen_of(width: u16, height: u16) -> (Rig, tempfile::TempDir) {
    let (mut rig, dir) = save_rig(width, height);
    rig.select_shell_call();
    rig.settle();
    (rig, dir)
}

#[test]
fn save_prompt_80x24_text() {
    let (mut rig, _dir) = screen_of(80, 24);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn save_prompt_80x24_styled() {
    let (mut rig, _dir) = screen_of(80, 24);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 80, 24)));
}

#[test]
fn export_prompt_120x40_text() {
    let (mut rig, _dir) = screen_of(120, 40);
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn overwrite_confirmation_80x24_text() {
    let (mut rig, dir) = screen_of(80, 24);
    let _path = existing(&mut rig, &dir);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn overwrite_confirmation_80x24_styled() {
    let (mut rig, dir) = screen_of(80, 24);
    let _path = existing(&mut rig, &dir);
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 80, 24)));
}

#[test]
fn exported_toast_80x24_text() {
    let (mut rig, _dir) = screen_of(80, 24);
    rig.ask(Key::Char('e'), KeyModifiers::NONE);
    rig.submit(EXPORT_DEFAULT);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn saved_toast_80x24_text() {
    let (mut rig, _dir) = screen_of(80, 24);
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit(SAVE_DEFAULT);
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn a_long_path_in_the_overwrite_question_keeps_the_end_of_the_question() {
    let (mut rig, dir) = screen_of(80, 24);
    let name = format!("{}.txt", "long-".repeat(20));
    std::fs::write(dir.path().join(&name), "old").unwrap();
    rig.ask(Key::Char('w'), KeyModifiers::NONE);
    rig.submit(&name);
    let rows = rows_of(&screen_buffer(&mut rig, 80, 24));
    let row = rows[23].trim_end();
    assert!(row.starts_with(" Overwrite …"), "{row}");
    assert!(row.ends_with("long-.txt? (y/n)"), "{row}");
}
