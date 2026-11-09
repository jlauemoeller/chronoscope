//! Resuming a session in its harness (product §5.7) through the app: `R` in the
//! browser and in the selector, the `y/n` question, the plan, the toasts and
//! the screens. The hand-off itself (`hand_off`) is tested with a recording
//! fake of its steps.
//!
//! The fixture sessions run in `/home/user/projects/brio`, which does not
//! exist here: most tests tell the app that every directory exists. The
//! tests of the missing directory use the real check.

use std::cell::RefCell;
use std::ffi::OsString;
use std::path::Path;
use std::rc::Rc;

use tuirealm::command::Direction;

use super::*;
use crate::app::resume::{ResumePlan, Steps, hand_off};
use crate::app::{Pane, SelectorMsg};
use crate::components::{MAX_TOAST_WIDTH, TOAST_CHROME};
use crate::config::Config;
use crate::model::SessionSummary;

const BRIO_CWD: &str = "/home/user/projects/brio";

fn plan(cwd: &str, program: &str, args: &[&str]) -> ResumePlan {
    ResumePlan {
        cwd: PathBuf::from(cwd),
        program: OsString::from(program),
        args: args.iter().map(OsString::from).collect(),
    }
}

fn claude_plan(session: &str) -> ResumePlan {
    plan(BRIO_CWD, "claude", &["--resume", session])
}

/// A rig of `width` x `height`; the harness has the default Claude command
/// and every directory exists.
fn resume_rig(width: u16, height: u16) -> Rig {
    let mut rig = rig_with(theme("default"), width, height);
    rig.harness.set_resume(Some("claude --resume {id}"));
    rig.app.set_dir_exists(|_| true);
    rig
}

/// Like [`resume_rig`], with the BRIO session open in the browser.
fn browser_rig(width: u16, height: u16) -> Rig {
    let mut rig = resume_rig(width, height);
    rig.open_loaded();
    rig
}

/// A selector over the fixture index with the newest session of BRIO
/// selected in Sessions.
fn selector_rig(width: u16, height: u16) -> Rig {
    let mut rig = resume_rig(width, height);
    rig.app
        .update(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)));
    rig
}

impl Rig {
    /// `R`, as a terminal sends it (shifted).
    fn press_resume(&mut self) {
        self.key(Key::Char('R'), KeyModifiers::SHIFT);
        self.settle();
    }

    fn resume_toast(&self) -> Option<String> {
        self.app.toast_text().map(str::to_owned)
    }

    fn prompt_is_mounted(&self) -> bool {
        self.app.application().mounted(&Id::Prompt)
    }

    /// The last row of the screen, without trailing blanks.
    fn last_row(&mut self, width: u16, height: u16) -> String {
        let rows = rows_of(&screen_buffer(self, width, height));
        rows[usize::from(height) - 1].trim().to_owned()
    }
}

// -- the question

#[test]
fn r_in_the_browser_asks_to_resume_the_open_session() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    assert!(rig.app.resume_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
    assert_eq!(rig.last_row(80, 24), "Resume 85a86657 in Claude? (y/n)");
    assert_eq!(rig.resume_toast(), None);
    // Asking is not doing.
    assert!(!rig.app.should_quit());
    assert_eq!(rig.app.take_resume_plan(), None);
}

#[test]
fn r_on_a_selected_session_in_the_selector_asks_about_that_session() {
    let mut rig = selector_rig(120, 40);
    assert_eq!(rig.app.selected_session().unwrap().as_str(), NEWER_SESSION);
    rig.press_resume();
    assert!(rig.app.resume_prompt_open());
    assert_eq!(rig.focus(), Some(Id::Prompt));
    assert_eq!(rig.last_row(120, 40), "");
    let screen = screen_text(&mut rig, 120, 40);
    assert!(
        screen.contains("Resume b2b2b2b2 in Claude? (y/n)"),
        "{screen}"
    );
}

#[test]
fn r_with_no_session_selected_does_nothing() {
    // Projects has the focus and no session is selected.
    let mut rig = resume_rig(120, 40);
    assert_eq!(rig.app.selected_session(), None);
    rig.press_resume();
    assert!(!rig.app.resume_prompt_open());
    assert!(!rig.prompt_is_mounted());
    assert_eq!(rig.resume_toast(), None);
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    // The request itself, for a session the selector does not list.
    rig.app.update(Msg::Output(OutputMsg::RequestResume(Some((
        CLAUDE,
        id("nope"),
    )))));
    assert!(!rig.prompt_is_mounted());
    assert_eq!(rig.resume_toast(), None);
}

#[test]
fn r_in_the_selector_over_the_browser_resumes_the_selected_session_not_the_open_one() {
    let mut rig = browser_rig(120, 40);
    rig.ctrl_o();
    rig.pump_until(|app| app.screen() == Screen::BrowserSelector);
    // The open session is selected; `k` moves to the newer one.
    assert_eq!(rig.app.selected_session().unwrap().as_str(), BRIO_SESSION);
    rig.press(Key::Char('k'));
    rig.settle();
    assert_eq!(rig.app.selected_session().unwrap().as_str(), NEWER_SESSION);
    rig.press_resume();
    let screen = screen_text(&mut rig, 120, 40);
    assert!(
        screen.contains("Resume b2b2b2b2 in Claude? (y/n)"),
        "{screen}"
    );
    rig.press(Key::Char('y'));
    rig.pump_until(App::should_quit);
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(NEWER_SESSION)));
}

#[test]
fn r_in_a_subagent_transcript_resumes_the_top_level_session() {
    let mut rig = super::subagent::subagent_rig(120, 40);
    rig.harness.set_resume(Some("claude --resume {id}"));
    rig.app.set_dir_exists(|_| true);
    rig.open_subagent(1);
    assert_eq!(rig.app.subagent_depth(), 1);
    rig.press_resume();
    let screen = screen_text(&mut rig, 120, 40);
    assert!(
        screen.contains("Resume 85a86657 in Claude? (y/n)"),
        "{screen}"
    );
    rig.press(Key::Char('y'));
    rig.pump_until(App::should_quit);
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(BRIO_SESSION)));
}

#[test]
fn r_with_the_projects_pane_focused_resumes_the_session_that_is_still_selected() {
    let mut rig = selector_rig(120, 40);
    // Sessions: select the older session, then go back to Projects.
    rig.app
        .update(Msg::Selector(SelectorMsg::SessionSelected(id(
            BRIO_SESSION,
        ))));
    rig.app
        .update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    assert_eq!(rig.focus(), Some(Id::ProjectList));
    assert_eq!(rig.app.selected_session().unwrap().as_str(), BRIO_SESSION);
    rig.press_resume();
    assert!(rig.app.resume_prompt_open());
    let screen = screen_text(&mut rig, 120, 40);
    assert!(
        screen.contains("Resume 85a86657 in Claude? (y/n)"),
        "{screen}"
    );
    rig.press(Key::Char('y'));
    rig.pump_until(App::should_quit);
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(BRIO_SESSION)));
}

#[test]
fn r_with_the_projects_pane_focused_and_a_new_project_selected_does_nothing() {
    let mut rig = selector_rig(120, 40);
    rig.app
        .update(Msg::Selector(SelectorMsg::Focus(Pane::Projects)));
    // Another project: its sessions are shown with none selected.
    rig.press(Key::Char('j'));
    rig.settle();
    assert_eq!(rig.app.selected_session(), None);
    rig.press_resume();
    assert!(!rig.app.resume_prompt_open());
    assert!(!rig.prompt_is_mounted());
    assert_eq!(rig.resume_toast(), None);
}

#[test]
fn a_question_whose_prompt_is_unmounted_by_opening_a_session_is_forgotten() {
    let mut rig = selector_rig(120, 40);
    rig.press_resume();
    assert!(rig.app.resume_prompt_open());
    rig.app.update(Msg::Browser(BrowserMsg::OpenSession(
        CLAUDE,
        id(BRIO_SESSION),
    )));
    assert!(!rig.prompt_is_mounted());
    assert!(!rig.app.resume_prompt_open());
    // A later prompt is not taken for the question: `y` in a search prompt
    // quits nothing.
    rig.harness.script(
        BRIO_SESSION,
        vec![ModelUpdate::Appended(turns()), ModelUpdate::Loaded],
    );
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.press(Key::Char('y'));
    rig.press(Key::Enter);
    rig.settle();
    assert!(!rig.app.should_quit());
    assert_eq!(rig.app.take_resume_plan(), None);
}

#[test]
fn r_does_nothing_while_another_prompt_is_open() {
    let mut rig = browser_rig(80, 24);
    rig.press(Key::Char('/'));
    rig.pump_until(|app| app.application().mounted(&Id::Prompt));
    rig.app.update(Msg::Output(OutputMsg::RequestResume(None)));
    assert!(!rig.app.resume_prompt_open());
    assert_eq!(rig.resume_toast(), None);
    // Typed in the search prompt, `R` is text.
    rig.press_resume();
    assert!(!rig.app.resume_prompt_open());
}

// -- the answer

#[test]
fn y_confirms_the_plan_and_quits() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    rig.press(Key::Char('y'));
    rig.pump_until(App::should_quit);
    assert!(!rig.prompt_is_mounted());
    assert!(!rig.app.resume_prompt_open());
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(BRIO_SESSION)));
    // It is given out once.
    assert_eq!(rig.app.take_resume_plan(), None);
}

/// The messages a prompt for another purpose than the resume question
/// sends, and those a `y/n` prompt never sends.
fn not_the_resume_answer() -> Vec<PromptMsg> {
    vec![
        PromptMsg::Submit(PromptPurpose::Resume, "y".to_owned()),
        PromptMsg::Changed(PromptPurpose::Resume, "y".to_owned()),
        PromptMsg::Nav(PromptPurpose::Resume, Direction::Down),
        PromptMsg::Cancel(PromptPurpose::Resume),
        PromptMsg::Confirm(PromptPurpose::Overwrite, true),
        PromptMsg::Submit(PromptPurpose::SelectorSearch, "y".to_owned()),
        PromptMsg::Submit(PromptPurpose::TranscriptSearch, "y".to_owned()),
        PromptMsg::Cancel(PromptPurpose::TranscriptSearch),
        PromptMsg::Submit(PromptPurpose::SavePath, "y".to_owned()),
        PromptMsg::Cancel(PromptPurpose::SavePath),
    ]
}

#[test]
fn only_the_answer_of_the_resume_prompt_answers_the_question() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    for msg in not_the_resume_answer() {
        rig.app.handle(Msg::Prompt(msg.clone()));
        assert!(rig.app.resume_prompt_open(), "{msg:?}");
        assert!(rig.prompt_is_mounted(), "{msg:?}");
        assert!(!rig.app.should_quit(), "{msg:?}");
    }
    rig.app
        .handle(Msg::Prompt(PromptMsg::Confirm(PromptPurpose::Resume, true)));
    assert!(rig.app.should_quit());
    assert!(!rig.prompt_is_mounted());
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(BRIO_SESSION)));
}

#[test]
fn a_no_from_the_resume_prompt_declines() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    rig.app.handle(Msg::Prompt(PromptMsg::Confirm(
        PromptPurpose::Resume,
        false,
    )));
    assert!(!rig.app.resume_prompt_open());
    assert!(!rig.prompt_is_mounted());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.app.should_quit());
    assert_eq!(rig.app.take_resume_plan(), None);
}

#[test]
fn capital_y_confirms_too() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    rig.key(Key::Char('Y'), KeyModifiers::SHIFT);
    rig.pump_until(App::should_quit);
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(BRIO_SESSION)));
}

#[test]
fn y_in_the_selector_confirms_instead_of_submitting_a_search() {
    let mut rig = selector_rig(120, 40);
    rig.press_resume();
    rig.press(Key::Char('y'));
    rig.pump_until(App::should_quit);
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(NEWER_SESSION)));
}

#[test]
fn n_esc_and_other_keys_cancel_and_return_the_focus_in_the_browser() {
    for (code, modifiers) in [
        (Key::Char('n'), KeyModifiers::NONE),
        (Key::Esc, KeyModifiers::NONE),
        (Key::Enter, KeyModifiers::NONE),
        (Key::Char('x'), KeyModifiers::NONE),
        (Key::Char('y'), KeyModifiers::CONTROL),
    ] {
        let mut rig = browser_rig(80, 24);
        rig.press_resume();
        assert!(rig.app.resume_prompt_open(), "{code:?}");
        rig.key(code, modifiers);
        rig.pump_until(|app| !app.resume_prompt_open());
        assert!(!rig.prompt_is_mounted(), "{code:?}");
        assert_eq!(rig.focus(), Some(Id::Transcript), "{code:?}");
        assert!(!rig.app.should_quit(), "{code:?}");
        assert_eq!(rig.app.take_resume_plan(), None, "{code:?}");
        assert_eq!(rig.resume_toast(), None, "{code:?}");
    }
}

#[test]
fn n_and_esc_cancel_and_return_the_focus_in_the_selector() {
    for code in [Key::Char('n'), Key::Esc] {
        let mut rig = selector_rig(120, 40);
        rig.press_resume();
        assert!(rig.app.resume_prompt_open(), "{code:?}");
        rig.press(code);
        rig.pump_until(|app| !app.resume_prompt_open());
        assert!(!rig.prompt_is_mounted(), "{code:?}");
        assert_eq!(rig.focus(), Some(Id::SessionList), "{code:?}");
        assert!(!rig.app.should_quit(), "{code:?}");
        assert_eq!(rig.app.take_resume_plan(), None, "{code:?}");
        // The selector is as it was, with no search started by the key.
        assert_eq!(rig.app.selected_session().unwrap().as_str(), NEWER_SESSION);
    }
}

#[test]
fn ctrl_c_in_the_question_quits_without_a_plan() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    rig.key(Key::Char('c'), KeyModifiers::CONTROL);
    rig.pump_until(App::should_quit);
    assert_eq!(rig.app.take_resume_plan(), None);
}

#[test]
fn a_question_cancelled_can_be_asked_again() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    rig.press(Key::Char('n'));
    rig.pump_until(|app| !app.resume_prompt_open());
    rig.press_resume();
    assert!(rig.app.resume_prompt_open());
    rig.press(Key::Char('y'));
    rig.pump_until(App::should_quit);
    assert_eq!(rig.app.take_resume_plan(), Some(claude_plan(BRIO_SESSION)));
}

// -- the command

#[test]
fn a_custom_template_is_honoured_in_the_plan() {
    let mut rig = browser_rig(80, 24);
    rig.harness
        .set_resume(Some("my-agent --continue \"{id}\" --flag={id} 'two words'"));
    rig.press_resume();
    rig.press(Key::Char('y'));
    rig.pump_until(App::should_quit);
    assert_eq!(
        rig.app.take_resume_plan(),
        Some(plan(
            BRIO_CWD,
            "my-agent",
            &[
                "--continue",
                BRIO_SESSION,
                &format!("--flag={BRIO_SESSION}"),
                "two words"
            ]
        ))
    );
}

fn summary_in(cwd: &str) -> SessionSummary {
    let project = project_in(CLAUDE, "brio", cwd, at(2026, 10, 28, 9, 30, 0));
    session(
        &project,
        BRIO_SESSION,
        "Investigate the camera profiles",
        at(2026, 10, 21, 8, 35, 15),
        at(2026, 10, 21, 9, 35, 15),
    )
}

#[test]
fn the_template_from_the_config_is_what_the_plan_is_made_of() {
    use crate::harness::claude::ClaudeHarness;
    use crate::harness::opencode::OpenCodeHarness;

    let summary = summary_in(BRIO_CWD);
    let mut config = Config::default();
    // The defaults.
    let claude = ClaudeHarness::new(&config, Path::new("/nonexistent"));
    let command = claude.resume_command(&summary).unwrap();
    assert_eq!(
        ResumePlan::new(&command, PathBuf::from(BRIO_CWD)),
        claude_plan(BRIO_SESSION)
    );
    let opencode = OpenCodeHarness::new(&config, Path::new("/nonexistent"));
    let command = opencode.resume_command(&summary).unwrap();
    assert_eq!(
        ResumePlan::new(&command, PathBuf::from(BRIO_CWD)),
        plan(BRIO_CWD, "opencode", &["--session", BRIO_SESSION])
    );
    // Custom templates.
    config.harness.claude.resume_command = "wrapper claude --resume {id} --verbose".to_owned();
    config.harness.opencode.resume_command = "oc -s {id}".to_owned();
    let claude = ClaudeHarness::new(&config, Path::new("/nonexistent"));
    let command = claude.resume_command(&summary).unwrap();
    assert_eq!(
        ResumePlan::new(&command, PathBuf::from(BRIO_CWD)),
        plan(
            BRIO_CWD,
            "wrapper",
            &["claude", "--resume", BRIO_SESSION, "--verbose"]
        )
    );
    let opencode = OpenCodeHarness::new(&config, Path::new("/nonexistent"));
    let command = opencode.resume_command(&summary).unwrap();
    assert_eq!(
        ResumePlan::new(&command, PathBuf::from(BRIO_CWD)),
        plan(BRIO_CWD, "oc", &["-s", BRIO_SESSION])
    );
}

#[test]
fn a_harness_without_a_resume_command_shows_a_toast_and_no_prompt() {
    let mut rig = browser_rig(80, 24);
    rig.harness.set_resume(None);
    rig.press_resume();
    assert_eq!(
        rig.resume_toast().as_deref(),
        Some("No resume command configured for Claude")
    );
    assert!(!rig.app.resume_prompt_open());
    assert!(!rig.prompt_is_mounted());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.app.should_quit());
}

#[test]
fn an_unusable_template_counts_as_no_command() {
    let mut rig = browser_rig(80, 24);
    rig.harness.set_resume(Some("   "));
    rig.press_resume();
    assert_eq!(
        rig.resume_toast().as_deref(),
        Some("No resume command configured for Claude")
    );
    assert!(!rig.prompt_is_mounted());
}

#[test]
fn a_selected_session_of_a_harness_that_is_not_loaded_has_no_command() {
    let mut rig = selector_rig(120, 40);
    rig.app.set_harnesses(Vec::<Arc<dyn Harness>>::new());
    rig.press_resume();
    assert_eq!(
        rig.resume_toast().as_deref(),
        Some("No resume command configured for Claude")
    );
    assert!(!rig.prompt_is_mounted());
}

// -- OpenCode

/// The two-tab app with OpenCode selected, its first session selected, and
/// the real OpenCode adapter (default `opencode --session {id}`).
fn opencode_rig() -> super::opencode::OpenCodeRig {
    let mut rig = super::opencode::two_tabs();
    rig.rig.app.set_dir_exists(|_| true);
    rig.rig.key(Key::Char('2'), KeyModifiers::NONE);
    rig.rig
        .pump_until(|app| app.selected_harness() == Some(HarnessId::OpenCode));
    rig.rig
        .app
        .update(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)));
    rig
}

#[test]
fn r_on_an_opencode_session_asks_about_opencode_and_plans_its_session_flag() {
    let mut rig = opencode_rig();
    let selected = rig.rig.app.selected_session().unwrap().clone();
    rig.rig.press_resume();
    let screen = screen_text(&mut rig.rig, 120, 40);
    let short: String = selected.as_str().chars().take(8).collect();
    assert!(
        screen.contains(&format!("Resume {short} in OpenCode? (y/n)")),
        "{screen}"
    );
    rig.rig.press(Key::Char('y'));
    rig.rig.pump_until(App::should_quit);
    let plan = rig.rig.app.take_resume_plan().unwrap();
    assert_eq!(plan.program, OsString::from("opencode"));
    assert_eq!(
        plan.args,
        [
            OsString::from("--session"),
            OsString::from(selected.as_str())
        ]
    );
    assert!(!plan.cwd.as_os_str().is_empty());
}

#[test]
fn an_opencode_without_a_command_says_so_by_its_name() {
    let mut rig = opencode_rig();
    let app = &mut rig.rig.app;
    let harness = Arc::new(
        crate::harness::opencode::OpenCodeHarness::with_db_path(
            std::path::PathBuf::from("/nonexistent/opencode.db"),
            Default::default(),
        )
        .with_resume_command(""),
    );
    // The claude fake stays; OpenCode now has an empty template.
    app.set_harnesses([
        Arc::clone(&rig.rig.harness) as Arc<dyn Harness>,
        harness as Arc<dyn Harness>,
    ]);
    rig.rig.press_resume();
    assert_eq!(
        rig.rig.resume_toast().as_deref(),
        Some("No resume command configured for OpenCode")
    );
    assert!(!rig.rig.prompt_is_mounted());
}

// -- the directory

/// A rig whose BRIO sessions run in `cwd`, with the real directory check.
fn rig_in(cwd: &str, width: u16, height: u16) -> Rig {
    let mut rig = rig_with(theme("default"), width, height);
    rig.harness.set_resume(Some("claude --resume {id}"));
    let project = project_in(CLAUDE, "brio", cwd, at(2026, 10, 28, 9, 30, 0));
    let mut sessions = SessionMap::new();
    sessions.insert(
        project.id.clone(),
        vec![session(
            &project,
            BRIO_SESSION,
            "Investigate the camera profiles",
            at(2026, 10, 21, 8, 35, 15),
            at(2026, 10, 21, 9, 35, 15),
        )],
    );
    rig.app.update(fixtures::message(snapshot_event(
        CLAUDE,
        true,
        vec![project],
        sessions,
    )));
    rig
}

fn assert_missing_toast(toast: &str, cwd: &Path) {
    assert!(toast.starts_with("Cannot resume: "), "{toast}");
    assert!(toast.ends_with(" no longer exists"), "{toast}");
    let name = cwd.file_name().unwrap().to_string_lossy();
    assert!(toast.contains(name.as_ref()), "{toast}");
}

#[test]
fn a_cwd_that_no_longer_exists_shows_a_toast_and_no_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().join("gone");
    std::fs::create_dir(&cwd).unwrap();
    let mut rig = rig_in(&cwd.display().to_string(), 80, 24);
    rig.harness.script(
        BRIO_SESSION,
        vec![ModelUpdate::Appended(turns()), ModelUpdate::Loaded],
    );
    rig.open(BRIO_SESSION);
    // While it exists the question is asked.
    rig.press_resume();
    assert!(rig.app.resume_prompt_open());
    rig.press(Key::Char('n'));
    rig.pump_until(|app| !app.resume_prompt_open());
    // Then the directory is removed.
    std::fs::remove_dir(&cwd).unwrap();
    rig.press_resume();
    assert_missing_toast(&rig.resume_toast().expect("a toast"), &cwd);
    assert!(!rig.app.resume_prompt_open());
    assert!(!rig.prompt_is_mounted());
    assert_eq!(rig.focus(), Some(Id::Transcript));
    assert!(!rig.app.should_quit());
    assert_eq!(rig.app.take_resume_plan(), None);
}

#[test]
fn a_cwd_that_is_a_file_is_not_a_directory_to_resume_in() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("file");
    std::fs::write(&file, "x").unwrap();
    let mut rig = rig_in(&file.display().to_string(), 80, 24);
    rig.app
        .update(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)));
    rig.press_resume();
    assert_missing_toast(&rig.resume_toast().expect("a toast"), &file);
    assert!(!rig.prompt_is_mounted());
}

#[test]
fn a_long_missing_cwd_is_cut_at_its_start_so_the_toast_fits() {
    let mut rig = resume_rig(80, 24);
    rig.app.set_dir_exists(|_| false);
    rig.open_loaded();
    rig.press_resume();
    let toast = rig.resume_toast().unwrap();
    assert_eq!(
        toast,
        "Cannot resume: /home/user/projects/brio no longer exists"
    );
    let long = "/very/long/directory/".repeat(5) + "project";
    let mut rig = rig_in(&long, 80, 24);
    rig.app
        .update(Msg::Selector(SelectorMsg::Focus(Pane::Sessions)));
    rig.press_resume();
    let toast = rig.resume_toast().unwrap();
    assert!(toast.starts_with("Cannot resume: …"), "{toast}");
    assert!(toast.ends_with("/project no longer exists"), "{toast}");
    let most = usize::from(MAX_TOAST_WIDTH - TOAST_CHROME);
    assert!(
        unicode_width::UnicodeWidthStr::width(toast.as_str()) <= most,
        "{toast}"
    );
}

#[test]
fn the_missing_cwd_toast_fits_the_box_of_a_narrow_terminal() {
    // At 60 columns the toast box is 56 wide: 52 for the text.
    let mut rig = resume_rig(60, 24);
    rig.open_loaded();
    rig.app.set_dir_exists(|_| false);
    rig.press_resume();
    let toast = rig.resume_toast().unwrap();
    assert_eq!(
        toast,
        "Cannot resume: …/user/projects/brio no longer exists"
    );
    assert_eq!(unicode_width::UnicodeWidthStr::width(toast.as_str()), 52);
    // The whole text is on the screen.
    let screen = screen_text(&mut rig, 60, 24);
    assert!(screen.contains(&toast), "{screen}");
}

// -- the hand-off

/// What the steps were asked, in order.
type Calls = Rc<RefCell<Vec<String>>>;

struct Recorder {
    calls: Calls,
    fail_change_dir: bool,
    run: Option<std::io::Result<i32>>,
}

impl Steps for Recorder {
    fn restore_terminal(&mut self) {
        self.calls.borrow_mut().push("restore".to_owned());
    }

    fn change_dir(&mut self, dir: &Path) -> std::io::Result<()> {
        self.calls
            .borrow_mut()
            .push(format!("cd {}", dir.display()));
        if self.fail_change_dir {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))
        } else {
            Ok(())
        }
    }

    fn run(&mut self, plan: &ResumePlan) -> std::io::Result<i32> {
        let args: Vec<_> = plan.args.iter().map(|a| a.to_string_lossy()).collect();
        self.calls.borrow_mut().push(format!(
            "run {} {}",
            plan.program.to_string_lossy(),
            args.join(" ")
        ));
        self.run.take().unwrap_or(Ok(0))
    }
}

fn recorder(run: Option<std::io::Result<i32>>, fail_change_dir: bool) -> (Recorder, Calls) {
    let calls = Calls::default();
    (
        Recorder {
            calls: Rc::clone(&calls),
            fail_change_dir,
            run,
        },
        calls,
    )
}

#[test]
fn the_terminal_is_restored_before_the_directory_is_changed_and_the_command_runs() {
    let (mut steps, calls) = recorder(Some(Ok(0)), false);
    let code = hand_off(&claude_plan(BRIO_SESSION), &mut steps);
    assert_eq!(code, 0);
    assert_eq!(
        *calls.borrow(),
        [
            "restore".to_owned(),
            format!("cd {BRIO_CWD}"),
            format!("run claude --resume {BRIO_SESSION}"),
        ]
    );
}

#[test]
fn the_exit_code_of_the_command_is_the_exit_code_of_the_process() {
    let (mut steps, _calls) = recorder(Some(Ok(42)), false);
    assert_eq!(hand_off(&claude_plan("x"), &mut steps), 42);
}

#[test]
fn a_directory_that_cannot_be_entered_ends_with_code_1_and_nothing_runs() {
    let (mut steps, calls) = recorder(None, true);
    assert_eq!(hand_off(&claude_plan("x"), &mut steps), 1);
    // The terminal was restored for the message; the command did not run.
    assert_eq!(
        *calls.borrow(),
        ["restore".to_owned(), format!("cd {BRIO_CWD}")]
    );
}

#[test]
fn a_command_that_cannot_be_started_ends_with_code_1() {
    let error = std::io::Error::from(std::io::ErrorKind::NotFound);
    let (mut steps, calls) = recorder(Some(Err(error)), false);
    assert_eq!(hand_off(&claude_plan("x"), &mut steps), 1);
    assert_eq!(calls.borrow().len(), 3);
}

#[cfg(unix)]
#[test]
fn the_real_steps_replace_the_process_only_on_unix_and_report_a_missing_program() {
    use crate::app::resume::SystemSteps;
    // A program that does not exist: `exec` returns the error.
    let restored = Rc::new(RefCell::new(0));
    let seen = Rc::clone(&restored);
    let mut steps = SystemSteps::new(move || *seen.borrow_mut() += 1);
    let error = steps
        .run(&plan("/", "chronoscope-no-such-program", &[]))
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    steps.restore_terminal();
    assert_eq!(*restored.borrow(), 1);
}

// -- the screens

#[test]
fn resume_confirmation_in_the_browser_80x24_text() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn resume_confirmation_in_the_browser_80x24_styled() {
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    insta::assert_snapshot!(buffer_to_styled(&screen_buffer(&mut rig, 80, 24)));
}

#[test]
fn resume_confirmation_in_the_selector_120x40_text() {
    let mut rig = selector_rig(120, 40);
    rig.press_resume();
    insta::assert_snapshot!(screen_text(&mut rig, 120, 40));
}

#[test]
fn the_missing_cwd_toast_over_the_browser_80x24_text() {
    let mut rig = resume_rig(80, 24);
    rig.open_loaded();
    rig.app.set_dir_exists(|_| false);
    rig.press_resume();
    insta::assert_snapshot!(screen_text(&mut rig, 80, 24));
}

#[test]
fn the_confirmation_names_and_the_harness_are_in_foreground_and_the_rest_in_help_text() {
    use crate::theme::Token;
    let mut rig = browser_rig(80, 24);
    rig.press_resume();
    let buffer = screen_buffer(&mut rig, 80, 24);
    let theme = theme("default");
    let help_text = theme.style(Token::HelpText).fg.unwrap();
    let foreground = theme.role(Role::Foreground);
    assert_ne!(help_text, foreground);
    let fg = |x| buffer[(x, 23)].fg;
    // One column of inset, then `Resume 85a86657 in Claude? (y/n)`.
    assert!((1..8).all(|x| fg(x) == help_text), "Resume ");
    assert!((8..16).all(|x| fg(x) == foreground), "the id");
    assert!((16..20).all(|x| fg(x) == help_text), " in ");
    assert!((20..26).all(|x| fg(x) == foreground), "Claude");
    assert!((26..33).all(|x| fg(x) == help_text), "? (y/n)");
}
