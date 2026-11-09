//! Resuming a session in its harness (product §5.7).
//!
//! 1. `R` in the browser, or on a selected session in the selector, answers
//!    [`Msg::RequestResume`](super::Msg::RequestResume). The browser's target
//!    is the open session, also while a subagent's transcript is shown.
//! 2. The `App` decides whether it can be done: the harness must have a
//!    resume command (else a toast says so), and the session's working
//!    directory must exist (else a toast says so). Then it opens the `y/n`
//!    [`Prompt`] in the help line: `Resume 85a86657 in Claude? (y/n)`.
//! 3. `y` stores a [`ResumePlan`] and quits. Any other key cancels.
//! 4. After the main loop, [`hand_off`] runs the plan: it restores the
//!    terminal, changes to the working directory and replaces the process by
//!    the harness's command (`exec` on Unix; spawn and wait elsewhere). The
//!    decision is pure; only [`SystemSteps`] touches the process.
//!
//! The command is never run through a shell.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use tuirealm::props::AttrValue;
use unicode_width::UnicodeWidthStr;

use super::{App, Id};
use crate::components::text_fit::truncate_start;
use crate::components::{MAX_TOAST_WIDTH, Prompt, RESUME_OPEN_ATTR, TOAST_CHROME};
use crate::model::{HarnessId, SessionId, SessionSummary};
use crate::platform::exec_or_run;
use crate::theme::Role;

/// The columns the prompt is inset from each side in the browser, like its
/// help line text.
const PROMPT_INSET: u16 = 1;

/// How many characters of the session id the question shows.
const SHORT_ID_CHARS: usize = 8;

/// What to run: the directory to change to, and the program with its
/// arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumePlan {
    /// The working directory of the session.
    pub cwd: PathBuf,
    /// The program to run, looked up in `PATH` as a shell would.
    pub program: OsString,
    /// Its arguments, with the session id filled in.
    pub args: Vec<OsString>,
}

impl ResumePlan {
    /// The plan that runs `command` in `cwd`.
    pub fn new(command: &Command, cwd: PathBuf) -> Self {
        Self {
            cwd,
            program: command.get_program().to_owned(),
            args: command.get_args().map(ToOwned::to_owned).collect(),
        }
    }
}

/// Tells whether a directory exists.
type DirCheck = Box<dyn Fn(&Path) -> bool>;

/// What the app knows about resuming: the plan waiting for its answer, and
/// the one that was confirmed.
#[derive(Default)]
pub(super) struct ResumeState {
    pending: Option<ResumePlan>,
    confirmed: Option<ResumePlan>,
    /// How the app asks whether a directory exists; `Path::is_dir` unless a
    /// test says otherwise.
    dir_exists: Option<DirCheck>,
}

impl ResumeState {
    /// Whether the `y/n` question is open.
    pub(super) fn pending(&self) -> bool {
        self.pending.is_some()
    }

    fn dir_exists(&self, dir: &Path) -> bool {
        self.dir_exists
            .as_ref()
            .map_or_else(|| dir.is_dir(), |f| f(dir))
    }
}

/// The first characters of `id`, as the question shows them.
fn short_id(id: &SessionId) -> String {
    id.as_str().chars().take(SHORT_ID_CHARS).collect()
}

/// `Cannot resume: <cwd> no longer exists`; a path too long for the toast is
/// cut at its start with `…`.
fn missing_dir_toast(cwd: &Path, screen_width: u16) -> String {
    const PREFIX: &str = "Cannot resume: ";
    const SUFFIX: &str = " no longer exists";
    // The toast box is at most `MAX_TOAST_WIDTH` wide, and 4 columns
    // narrower than the screen.
    let box_width = MAX_TOAST_WIDTH.min(screen_width.saturating_sub(4));
    let room = usize::from(box_width.saturating_sub(TOAST_CHROME))
        .saturating_sub(PREFIX.width() + SUFFIX.width());
    format!(
        "{PREFIX}{}{SUFFIX}",
        truncate_start(&cwd.display().to_string(), room)
    )
}

impl App {
    /// Replaces how the app checks that a session's directory exists (tests
    /// give a fixed answer).
    #[doc(hidden)]
    pub fn set_dir_exists(&mut self, exists: impl Fn(&Path) -> bool + 'static) {
        self.resume.dir_exists = Some(Box::new(exists));
    }

    /// Whether the `Resume … (y/n)` question is open.
    pub fn resume_prompt_open(&self) -> bool {
        self.resume.pending()
    }

    /// The plan the user confirmed, which the main loop runs once the
    /// terminal is restored. It is given out once.
    pub fn take_resume_plan(&mut self) -> Option<ResumePlan> {
        self.resume.confirmed.take()
    }

    /// The summary of the session to resume: the one the selector selected,
    /// or the open session (never a subagent's).
    fn resume_target(
        &self,
        target: Option<(HarnessId, SessionId)>,
    ) -> Option<(HarnessId, SessionSummary)> {
        match target {
            Some((harness, id)) => {
                let summary = self.selector.find_session(harness, &id)?;
                Some((harness, summary.clone()))
            }
            None => {
                let open = self.browser.open.as_ref()?;
                Some((open.harness, open.summary.clone()))
            }
        }
    }

    /// `R`: checks that the session can be resumed and asks for
    /// confirmation. Does nothing while a prompt or an overlay is open.
    pub(super) fn request_resume(&mut self, target: Option<(HarnessId, SessionId)>) {
        let busy = self.application.mounted(&Id::Prompt)
            || self.help_size.is_some()
            || self.details_size.is_some()
            || self.theme_selector.is_some();
        if busy {
            return;
        }
        let Some((harness, summary)) = self.resume_target(target) else {
            return;
        };
        let command = self
            .harnesses
            .get(&harness)
            .and_then(|h| h.resume_command(&summary));
        let Some(command) = command else {
            self.toast_quiet(format!(
                "No resume command configured for {}",
                harness.display_name()
            ));
            return;
        };
        let cwd = summary.cwd.clone().or_else(|| {
            self.selector
                .find_project(harness, &summary.project)
                .and_then(|project| project.cwd.clone())
        });
        let Some(cwd) = cwd else {
            self.toast_quiet("Cannot resume: the working directory is unknown");
            return;
        };
        if !self.resume.dir_exists(&cwd) {
            self.toast_quiet(missing_dir_toast(&cwd, self.size.0));
            return;
        }
        let mut prompt = Prompt::confirm_two(
            "Resume ",
            &short_id(&summary.id),
            " in ",
            harness.display_name(),
            "? (y/n)",
            self.theme.clone(),
        );
        // In the browser the prompt is on its help line; over the selector it
        // is on the selector's, like its search.
        if !self.screen.has_selector() {
            prompt = prompt.with_fill(Role::Background).inset(PROMPT_INSET);
        }
        match self.open_overlay(Id::Prompt, Box::new(prompt)) {
            Ok(()) => {
                self.resume.pending = Some(ResumePlan::new(&command, cwd));
                self.push_resume_open();
            }
            Err(err) => log::warn!("could not open the resume prompt: {err}"),
        }
    }

    /// Tells the selector's frame whether the question is open: while it
    /// is, a press outside the selector does nothing.
    fn push_resume_open(&mut self) {
        let open = self.resume.pending();
        self.set_attr(Id::SelectorFrame, RESUME_OPEN_ATTR, AttrValue::Flag(open));
    }

    /// `y` or another key in the question: `y` confirms the plan and quits;
    /// anything else cancels.
    pub(super) fn resume_answered(&mut self, answer: &str) {
        let Some(plan) = self.resume.pending.take() else {
            return;
        };
        self.push_resume_open();
        self.close_overlay(Id::Prompt);
        if answer == "y" {
            self.resume.confirmed = Some(plan);
            self.quit = true;
        }
    }

    /// Forgets the question without touching the prompt, for when the prompt
    /// is unmounted by someone else.
    pub(super) fn clear_resume_question(&mut self) {
        if self.resume.pending.take().is_some() {
            self.push_resume_open();
        }
    }

    /// `n`, `Esc` or any other key but `y`: nothing is resumed and the focus
    /// returns to where it was.
    pub(super) fn resume_declined(&mut self) {
        if self.resume.pending.take().is_some() {
            self.push_resume_open();
            self.close_overlay(Id::Prompt);
        }
    }
}

/// The steps that hand the process over; [`SystemSteps`] does them for real,
/// tests record them.
pub trait Steps {
    /// Puts the terminal back as it was before Chronoscope started.
    fn restore_terminal(&mut self);
    /// Changes the working directory of the process.
    fn change_dir(&mut self, dir: &Path) -> std::io::Result<()>;
    /// Runs the plan in place of the process. On Unix this only returns when
    /// the program could not be started; elsewhere it runs the program to its
    /// end and returns its exit code.
    fn run(&mut self, plan: &ResumePlan) -> std::io::Result<i32>;
}

/// Runs `plan` with `steps`, in the order that matters: the terminal is
/// restored first (the harness needs it as it was), then the directory is
/// changed, then the command runs. Returns the exit code of the process: 1
/// with a message on stderr when a step failed, else the child's.
pub fn hand_off(plan: &ResumePlan, steps: &mut dyn Steps) -> i32 {
    steps.restore_terminal();
    if let Err(err) = steps.change_dir(&plan.cwd) {
        eprintln!(
            "chronoscope: cannot change to {}: {err}",
            plan.cwd.display()
        );
        return 1;
    }
    match steps.run(plan) {
        Ok(code) => code,
        Err(err) => {
            eprintln!(
                "chronoscope: cannot run {}: {err}",
                plan.program.to_string_lossy()
            );
            1
        }
    }
}

/// The real steps. The terminal is restored by calling `restore`.
pub struct SystemSteps<R: FnMut()> {
    restore: R,
}

impl<R: FnMut()> SystemSteps<R> {
    /// Steps that restore the terminal with `restore`.
    pub fn new(restore: R) -> Self {
        Self { restore }
    }
}

impl<R: FnMut()> Steps for SystemSteps<R> {
    fn restore_terminal(&mut self) {
        (self.restore)();
    }

    fn change_dir(&mut self, dir: &Path) -> std::io::Result<()> {
        std::env::set_current_dir(dir)
    }

    /// `exec` on Unix, a child process waited for elsewhere
    /// ([`exec_or_run`]).
    fn run(&mut self, plan: &ResumePlan) -> std::io::Result<i32> {
        // Nothing buffered is lost when the process image is replaced.
        log::logger().flush();
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let mut command = Command::new(&plan.program);
        command.args(&plan.args);
        exec_or_run(command)
    }
}
