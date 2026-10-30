//! The session browser's part of the application (SPEC §1.2, §1.4, §4.1,
//! §4.2): opening a session, the updates of its load, and the selector over
//! the browser.
//!
//! Opening a session loads it on a worker through its harness. The worker's
//! [`ModelUpdate`](crate::model::ModelUpdate)s come back as
//! [`UserEvent::Session`](super::UserEvent::Session), tagged with the number
//! of the load (its *generation*), and [`App::session_update`] forwards the
//! turns to the `Transcript` through its `attr`. The `App` keeps no copy of
//! them.

use std::ops::{Deref, DerefMut};
use std::time::SystemTime;

use chrono::{DateTime, Utc};
use tuirealm::props::{AttrValue, PropPayload};

use super::hints::{browser_hints, browser_hints_at};
use super::layout::Screen;
use super::redraw::RedrawRequest;
use super::selector::{SELECTOR_IDS, mount_selector};
use super::session_port::{SessionUpdate, forward_to};
use super::{App, Id};
use crate::components::turn_block::export::ExportMeta;
use crate::components::{
    BREADCRUMB_ATTR, BrowserHeader, EMPTY_SESSION, EmptyState, HINTS_ATTR, HelpLine, LIVE_ATTR,
    MODEL_UPDATE_ATTR, NEW_SECTIONS_ATTR, OPEN_SUBAGENT_KEY_ATTR, PARSE_WARNINGS_ATTR,
    SESSION_ID_ATTR, STATUS_ATTR, TRUNCATE_OUTPUT_LINES_ATTR, TURN_COUNT_ATTR, Transcript,
    hints_value, payload_value, status_value,
};
use crate::harness::WatchHandle;
use crate::keymap::{Action, Context};
use crate::model::{HarnessId, ModelUpdate, SessionId, SessionSummary, ToolCallId, is_active_at};
use crate::theme::Role;

/// The browser's components, in drawing order.
const BROWSER_IDS: [Id; 4] = [
    Id::BrowserHeader,
    Id::Transcript,
    Id::BrowserHelpLine,
    Id::EmptySession,
];

/// The toast when a subagent section has no transcript to open.
pub const NO_SUBAGENT_TRANSCRIPT: &str = "No transcript for this subagent";

/// The columns the browser's help line text is inset from each side.
const HELP_LINE_INSET: u16 = 1;

/// The session that is open.
#[derive(Debug, Clone)]
pub(super) struct OpenSession {
    pub(super) harness: HarnessId,
    pub(super) summary: SessionSummary,
}

/// What the browser knows about one loaded session: the open one (depth 0)
/// or a subagent's transcript opened over it.
#[derive(Debug, Default)]
pub(super) struct LoadView {
    /// The number of the load. Updates of any other are dropped.
    generation: u64,
    /// Keeps the load's worker alive; dropping it stops the worker.
    watch: Option<WatchHandle>,
    /// Whether the initial parse is complete.
    loaded: bool,
    /// Whether the session is loaded and has no turns.
    empty: bool,
    /// The percentage the initial load reports, while it runs.
    loading: Option<u32>,
    /// The sections the transcript counts as new (`↓ N new`).
    new_sections: usize,
    /// The latest `Activity`: when the data last changed, and the harness's
    /// idle flag.
    activity: Option<(SystemTime, Option<bool>)>,
    /// Whether the header shows `● live`.
    live: bool,
    /// How many entries of the session could not be parsed.
    warnings: usize,
    /// The search of the transcript.
    pub(super) search: super::transcript_search::BrowserSearch,
}

/// A subagent's transcript opened over the session or over another subagent.
#[derive(Debug)]
struct Level {
    /// The subagent's session.
    id: SessionId,
    /// The agent in the breadcrumb.
    label: String,
    view: LoadView,
}

/// The browser's domain state. The turns are not part of it: the
/// `Transcript`s own them.
///
/// It reads and writes the view of the transcript on top (the deepest
/// subagent, else the session) through `Deref`: that is the one the header
/// and the help line show.
#[derive(Debug, Default)]
pub(super) struct BrowserState {
    /// The open session, if any.
    pub(super) open: Option<OpenSession>,
    /// The session's own view (depth 0).
    root: LoadView,
    /// The subagent transcripts opened one over the other; the transcript at
    /// depth `n` is `levels[n - 1]`.
    levels: Vec<Level>,
    /// The number of the latest load started. Never goes back, so the
    /// number of a load that was stopped is never given to another.
    load_seq: u64,
    /// The save or export waiting for its path or for an overwrite to be
    /// confirmed.
    pub(super) write: Option<super::save::PendingWrite>,
}

impl Deref for BrowserState {
    type Target = LoadView;

    fn deref(&self) -> &LoadView {
        self.levels.last().map_or(&self.root, |level| &level.view)
    }
}

impl DerefMut for BrowserState {
    fn deref_mut(&mut self) -> &mut LoadView {
        self.levels
            .last_mut()
            .map_or(&mut self.root, |level| &mut level.view)
    }
}

impl BrowserState {
    /// Whether the transcript is replaced by "Session file is empty".
    pub(super) fn session_empty(&self) -> bool {
        self.empty
    }

    /// Tells the workers of every load to stop.
    pub(super) fn stop_watching(&mut self) {
        self.root.watch = None;
        for level in &mut self.levels {
            level.view.watch = None;
        }
    }

    /// The depth of the transcript on top: 0 for the session.
    pub(super) fn depth(&self) -> usize {
        self.levels.len()
    }

    /// The view at `depth`.
    fn view_at_mut(&mut self, depth: usize) -> Option<&mut LoadView> {
        match depth.checked_sub(1) {
            None => Some(&mut self.root),
            Some(n) => self.levels.get_mut(n).map(|level| &mut level.view),
        }
    }

    /// The depth whose load has number `generation`.
    fn depth_of(&self, generation: u64) -> Option<usize> {
        (0..=self.levels.len()).find(|&depth| {
            let view = match depth.checked_sub(1) {
                None => &self.root,
                Some(n) => &self.levels[n].view,
            };
            view.generation == generation
        })
    }

    /// The session of the transcript on top.
    pub(super) fn current_session(&self) -> Option<&SessionId> {
        match self.levels.last() {
            Some(level) => Some(&level.id),
            None => self.open.as_ref().map(|open| &open.summary.id),
        }
    }

    /// The agents of the open subagents, outermost first.
    fn breadcrumb(&self) -> Vec<String> {
        self.levels
            .iter()
            .map(|level| level.label.clone())
            .collect()
    }
}

/// The component that shows the transcript at `depth`.
pub(super) fn transcript_id(depth: usize) -> Id {
    if depth == 0 {
        Id::Transcript
    } else {
        Id::Subagent(depth)
    }
}

impl App {
    // -- read access, for tests

    /// The screen that is shown.
    pub fn screen(&self) -> Screen {
        self.screen
    }

    /// The harness and id of the open session.
    pub fn open_session_id(&self) -> Option<(HarnessId, &SessionId)> {
        let open = self.browser.open.as_ref()?;
        Some((open.harness, &open.summary.id))
    }

    /// The number of the load of the transcript on top: it grows with every
    /// session and subagent opened.
    pub fn session_generation(&self) -> u64 {
        self.browser.generation
    }

    /// How many subagent transcripts are open one over the other: 0 while
    /// the session itself is shown.
    pub fn subagent_depth(&self) -> usize {
        self.browser.depth()
    }

    /// The agents of the open subagents, outermost first (the breadcrumb).
    pub fn breadcrumb(&self) -> Vec<String> {
        self.browser.breadcrumb()
    }

    /// The session of the transcript on top: the open session, or a
    /// subagent's.
    pub fn top_session_id(&self) -> Option<&SessionId> {
        self.browser.current_session()
    }

    /// The component that shows the transcript on top.
    pub(super) fn top_transcript(&self) -> Id {
        transcript_id(self.browser.depth())
    }

    /// Whether the initial parse of the open session is complete.
    pub fn session_loaded(&self) -> bool {
        self.browser.loaded
    }

    /// Whether "Session file is empty" replaces the transcript.
    pub fn session_is_empty(&self) -> bool {
        self.browser.empty
    }

    // -- wiring

    /// Hands the app the harnesses sessions are loaded from.
    pub fn set_harnesses(
        &mut self,
        harnesses: impl IntoIterator<Item = std::sync::Arc<dyn crate::harness::Harness>>,
    ) {
        self.harnesses = harnesses.into_iter().map(|h| (h.id(), h)).collect();
    }

    /// Hands the app the channel the updates of a session load are sent to;
    /// a [`SessionPort`](super::SessionPort) reads its other end.
    pub fn set_session_sink(&mut self, sink: std::sync::mpsc::Sender<super::SessionEvent>) {
        self.session_sink = Some(sink);
    }

    // -- opening a session

    /// Opens `id` of `harness`: loads it on a worker, replaces the selector
    /// (or the browser of the session that was open) by the new browser and
    /// focuses the transcript. Opening the open session just closes the
    /// selector. A session the index does not list is ignored.
    pub(super) fn open_session(&mut self, harness: HarnessId, id: &SessionId) {
        let is_open = self
            .browser
            .open
            .as_ref()
            .is_some_and(|open| open.harness == harness && &open.summary.id == id);
        if is_open {
            self.close_selector();
            return;
        }
        let Some(summary) = self.selector.find_session(harness, id).cloned() else {
            log::warn!("cannot open {id}: the index does not list it");
            return;
        };
        let (Some(loader), Some(sink)) = (
            self.harnesses.get(&harness).cloned(),
            self.session_sink.clone(),
        ) else {
            log::error!("cannot open {id}: no {harness} harness or session channel");
            self.toast_quiet("Cannot open the session");
            return;
        };
        let generation = self.browser.load_seq + 1;
        // The worker of the session that was open stops before the new load
        // starts. If the load fails the old browser stays, no longer live.
        self.browser.stop_watching();
        let watch = match loader.load(id, forward_to(generation, sink)) {
            Ok(watch) => watch,
            Err(err) => {
                log::warn!("could not load {id}: {err}");
                self.toast_quiet(format!("Could not open the session: {err}"));
                return;
            }
        };
        // The subagent transcripts are closed with their loads.
        self.unmount_subagents();
        self.browser = super::browser::BrowserState {
            open: Some(OpenSession { harness, summary }),
            root: LoadView {
                generation,
                watch: Some(watch),
                ..LoadView::default()
            },
            load_seq: generation,
            ..BrowserState::default()
        };
        self.unmount_selector();
        self.unmount_browser();
        self.focus_stack.clear();
        self.screen = Screen::Browser;
        if let Err(err) = self.mount_browser() {
            log::error!("could not mount the browser: {err}");
        }
        self.push_viewports();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Shows a toast; when that fails the failure is only logged.
    pub(super) fn toast_quiet(&mut self, text: impl Into<String>) {
        if let Err(err) = self.show_toast(text) {
            log::warn!("could not show a toast: {err}");
        }
    }

    /// The display name of the open session's project: the project's name,
    /// else the last part of its id.
    pub(super) fn open_project_name(&self) -> Option<String> {
        let open = self.browser.open.as_ref()?;
        let project = self
            .selector
            .find_project(open.harness, &open.summary.project);
        Some(project.map_or_else(
            || {
                let id = open.summary.project.as_str();
                id.rsplit('/')
                    .find(|part| !part.is_empty())
                    .unwrap_or(id)
                    .to_owned()
            },
            |project| project.name.clone(),
        ))
    }

    /// The project name and the working directory the header shows for the
    /// open session.
    fn header_texts(&self) -> (String, String) {
        let Some(open) = self.browser.open.as_ref() else {
            return (String::new(), String::new());
        };
        let project = self
            .selector
            .find_project(open.harness, &open.summary.project);
        let cwd = open
            .summary
            .cwd
            .as_ref()
            .or_else(|| project.and_then(|p| p.cwd.as_ref()))
            .map(|cwd| cwd.display().to_string())
            .unwrap_or_default();
        (self.open_project_name().unwrap_or_default(), cwd)
    }

    /// A transcript for the session `session_id` of the open session's
    /// project, titled `title` in an export.
    fn new_transcript(&self, session_id: &str, title: String, nested: bool) -> Option<Transcript> {
        let open = self.browser.open.as_ref()?;
        let (project, cwd) = self.header_texts();
        let meta = ExportMeta {
            title,
            project,
            session_id: session_id.to_owned(),
            cwd,
            started: open.summary.started_at,
            harness: open.harness.display_name().to_owned(),
        };
        Some(
            Transcript::new(self.theme.clone(), self.timestamps.clone())
                .keymap(self.keymap.clone())
                .export_meta(meta)
                .dim_unselected(self.config.dim_unselected)
                .show_system(self.config.show_system)
                .nested(nested)
                .loading(!self.browser.loaded),
        )
    }

    /// Gives the transcript `id` the settings that are not set when it is
    /// built.
    fn configure_transcript(&mut self, id: Id) {
        self.set_attr(
            id,
            TRUNCATE_OUTPUT_LINES_ATTR,
            AttrValue::Length(self.config.truncate_output_lines),
        );
        if let Some(label) = self.keymap.label(Context::Browser, Action::OpenSubagent) {
            self.set_attr(id, OPEN_SUBAGENT_KEY_ATTR, AttrValue::String(label));
        }
    }

    /// Mounts the header, the transcript and the help line of the open
    /// session, and gives the transcript the focus.
    fn mount_browser(&mut self) -> Result<(), tuirealm::application::ApplicationError> {
        let Some(open) = self.browser.open.as_ref() else {
            return Ok(());
        };
        let (name, cwd) = self.header_texts();
        let title = open.summary.title.clone();
        let session_id = open.summary.id.as_str().to_owned();
        let header = BrowserHeader::new(name, &session_id, cwd, self.theme.clone());
        let Some(transcript) = self.new_transcript(&session_id, title, false) else {
            return Ok(());
        };
        let help = HelpLine::new(
            browser_hints(&self.keymap),
            Vec::new(),
            self.theme.clone(),
            Role::Background,
        )
        .inset(HELP_LINE_INSET);
        let empty = EmptyState::new(EMPTY_SESSION, self.theme.clone());
        self.application
            .mount(Id::BrowserHeader, Box::new(header), Vec::new())?;
        self.application
            .mount(Id::Transcript, Box::new(transcript), Vec::new())?;
        self.application
            .mount(Id::BrowserHelpLine, Box::new(help), Vec::new())?;
        self.application
            .mount(Id::EmptySession, Box::new(empty), Vec::new())?;
        self.configure_transcript(Id::Transcript);
        self.application.active(&Id::Transcript)
    }

    fn unmount_browser(&mut self) {
        for id in BROWSER_IDS {
            if self.application.mounted(&id)
                && let Err(err) = self.application.umount(&id)
            {
                log::warn!("could not unmount {id:?}: {err}");
            }
        }
    }

    /// Unmounts every subagent transcript.
    fn unmount_subagents(&mut self) {
        for depth in 1..=self.browser.depth() {
            let id = Id::Subagent(depth);
            if self.application.mounted(&id)
                && let Err(err) = self.application.umount(&id)
            {
                log::warn!("could not unmount {id:?}: {err}");
            }
        }
    }

    // -- subagent transcripts (SPEC §5.4)

    /// `a` on a subagent section: asks the harness for the transcript of the
    /// subagent that tool call `call` started and mounts a `Transcript` for
    /// it over the one on top, which stays mounted but is not drawn. The new
    /// one is loaded and watched like any session and gets the focus. With
    /// no transcript to open a toast says so and nothing is mounted.
    pub(super) fn open_subagent(&mut self, call: &ToolCallId, label: String) {
        if self.screen != Screen::Browser || self.application.mounted(&Id::Prompt) {
            return;
        }
        let Some(harness) = self.browser.open.as_ref().map(|open| open.harness) else {
            return;
        };
        let Some(parent) = self.browser.current_session().cloned() else {
            return;
        };
        let (Some(loader), Some(sink)) = (
            self.harnesses.get(&harness).cloned(),
            self.session_sink.clone(),
        ) else {
            log::error!("cannot open a subagent: no {harness} harness or session channel");
            self.toast_quiet("Cannot open the subagent");
            return;
        };
        let Some(id) = loader.subagent(&parent, call) else {
            self.toast_quiet(NO_SUBAGENT_TRANSCRIPT);
            return;
        };
        // The number is taken before the worker starts, so it is never given
        // again, whatever happens next.
        let generation = self.browser.load_seq + 1;
        self.browser.load_seq = generation;
        let watch = match loader.load(&id, forward_to(generation, sink)) {
            Ok(watch) => watch,
            Err(err) => {
                log::warn!("could not load {id}: {err}");
                self.toast_quiet(format!("Could not open the subagent: {err}"));
                return;
            }
        };
        let depth = self.browser.depth() + 1;
        let mounted = self.mount_subagent(depth, &id, &label);
        if let Err(err) = mounted {
            log::error!("could not mount the subagent transcript: {err}");
            self.toast_quiet("Cannot open the subagent");
            return;
        }
        // An answer to a search of the covered transcript would land in the
        // subagent's view and be dropped: forget that it is waited for.
        self.browser.search.forget_pending();
        self.browser.levels.push(Level {
            id,
            label,
            view: LoadView {
                generation,
                watch: Some(watch),
                ..LoadView::default()
            },
        });
        self.show_top_in_header();
        self.refresh_status();
        self.push_viewports();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Mounts the transcript of subagent `id` at `depth` and focuses it.
    fn mount_subagent(
        &mut self,
        depth: usize,
        id: &SessionId,
        label: &str,
    ) -> Result<(), tuirealm::application::ApplicationError> {
        let component = Id::Subagent(depth);
        let transcript = self
            .new_transcript(id.as_str(), format!("Agent: {label}"), true)
            .map(|transcript| transcript.loading(true));
        let Some(transcript) = transcript else {
            return Ok(());
        };
        self.application
            .mount(component, Box::new(transcript), Vec::new())?;
        self.configure_transcript(component);
        if let Err(err) = self.application.active(&component) {
            self.application.umount(&component)?;
            return Err(err);
        }
        Ok(())
    }

    /// The back key in a subagent's transcript: drops its load, unmounts it
    /// and returns to the transcript it was opened from, which was never
    /// touched, with the header and help line as that one had them. At depth
    /// 0, or while a prompt is open, nothing happens.
    pub(super) fn close_subagent(&mut self) {
        if self.screen != Screen::Browser
            || self.application.mounted(&Id::Prompt)
            || self.browser.levels.is_empty()
        {
            return;
        }
        let depth = self.browser.depth();
        // Dropping the level stops its worker; its late updates are dropped
        // because no load has its number.
        drop(self.browser.levels.pop());
        let id = Id::Subagent(depth);
        if let Err(err) = self.application.umount(&id) {
            log::warn!("could not unmount {id:?}: {err}");
        }
        let top = self.top_transcript();
        if let Err(err) = self.application.active(&top) {
            log::warn!("could not return the focus to {top:?}: {err}");
        }
        self.show_top_in_header();
        // What the transcript on top says about itself, as it is now: it kept
        // receiving its updates while it was covered.
        self.sync_search_position();
        self.sync_new_sections();
        self.refresh_session_empty();
        self.refresh_live();
        self.push_viewports();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Shows the breadcrumb, the session id, the parse warnings and the live
    /// marker of the transcript on top in the header, and its status in the
    /// help line.
    fn show_top_in_header(&mut self) {
        let breadcrumb = self.browser.breadcrumb();
        let session = self
            .browser
            .current_session()
            .map(|id| id.as_str().to_owned())
            .unwrap_or_default();
        let (warnings, live) = (self.browser.warnings, self.browser.live);
        self.set_attr(
            Id::BrowserHeader,
            BREADCRUMB_ATTR,
            payload_value(breadcrumb),
        );
        self.set_attr(
            Id::BrowserHeader,
            SESSION_ID_ATTR,
            AttrValue::String(session),
        );
        self.set_attr(
            Id::BrowserHeader,
            PARSE_WARNINGS_ATTR,
            AttrValue::Length(warnings),
        );
        self.set_attr(Id::BrowserHeader, LIVE_ATTR, AttrValue::Flag(live));
        self.set_attr(
            Id::BrowserHelpLine,
            HINTS_ATTR,
            hints_value(browser_hints_at(&self.keymap, self.browser.depth())),
        );
        self.refresh_status();
    }

    // -- the updates of the load

    /// Applies what the load of generation `generation` reported. Updates of
    /// a load that is not running (replaced, or a subagent that was closed)
    /// are dropped.
    pub(super) fn session_update(&mut self, generation: u64, update: &SessionUpdate) {
        let depth = self
            .browser
            .open
            .as_ref()
            .and_then(|_| self.browser.depth_of(generation));
        let Some(depth) = depth else {
            log::debug!(
                "dropping an update of load {generation}; the current one is {}",
                self.browser.generation
            );
            return;
        };
        self.request_redraw(RedrawRequest::Dirty);
        if depth != self.browser.depth() {
            self.covered_session_update(depth, update);
            return;
        }
        match update.update() {
            ModelUpdate::Progress(fraction) => {
                if !self.browser.loaded {
                    self.set_loading(Some(percent(*fraction)));
                }
            }
            ModelUpdate::Loaded => {
                self.browser.loaded = true;
                self.set_loading(None);
                // The transcript decides whether to follow from here on.
                self.forward_to_transcript(depth, update);
                self.sync_new_sections();
                self.refresh_session_empty();
            }
            ModelUpdate::ParseWarnings(count) => {
                self.browser.warnings = *count;
                self.set_attr(
                    Id::BrowserHeader,
                    PARSE_WARNINGS_ATTR,
                    AttrValue::Length(*count),
                );
            }
            ModelUpdate::Appended(_) => {
                self.forward_to_transcript(depth, update);
                self.sync_new_sections();
                self.refresh_session_empty();
            }
            ModelUpdate::Updated(_) => {
                self.forward_to_transcript(depth, update);
                self.sync_new_sections();
            }
            ModelUpdate::Reset => {
                self.forward_to_transcript(depth, update);
                self.sync_new_sections();
                // The matches were of the turns that are gone.
                self.search_reloaded();
            }
            ModelUpdate::Activity {
                last_modified,
                idle,
            } => {
                self.browser.activity = Some((*last_modified, *idle));
                self.refresh_live();
            }
        }
    }

    /// Applies an update of the load at `depth`, which a subagent transcript
    /// covers: its transcript gets the turns and its view the facts, but the
    /// header and the help line are those of the transcript on top. They are
    /// brought up to date when the covering one is closed.
    fn covered_session_update(&mut self, depth: usize, update: &SessionUpdate) {
        let Some(view) = self.browser.view_at_mut(depth) else {
            return;
        };
        let forward = match update.update() {
            ModelUpdate::Progress(fraction) => {
                if !view.loaded {
                    view.loading = Some(percent(*fraction));
                }
                false
            }
            ModelUpdate::Loaded => {
                view.loaded = true;
                view.loading = None;
                true
            }
            ModelUpdate::ParseWarnings(count) => {
                view.warnings = *count;
                false
            }
            ModelUpdate::Activity {
                last_modified,
                idle,
            } => {
                view.activity = Some((*last_modified, *idle));
                false
            }
            ModelUpdate::Appended(_) | ModelUpdate::Updated(_) | ModelUpdate::Reset => true,
        };
        if forward {
            self.forward_to_transcript(depth, update);
        }
    }

    /// Re-evaluates whether the session is live (SPEC §7.2) and pushes the
    /// result to the header when it changed. Runs on every `Tick`, so the
    /// marker goes out once the session has been quiet for a minute.
    pub(super) fn refresh_live(&mut self) {
        let live = self.browser.activity.is_some_and(|(modified, idle)| {
            is_active_at(DateTime::<Utc>::from(modified), idle, (self.clock)())
        });
        if live != self.browser.live {
            self.browser.live = live;
            self.set_attr(Id::BrowserHeader, LIVE_ATTR, AttrValue::Flag(live));
        }
    }

    /// Whether the header shows `● live`.
    pub fn session_is_live(&self) -> bool {
        self.browser.live
    }

    /// The number the help line's `↓ N new` shows (0: nothing).
    pub fn new_sections(&self) -> usize {
        self.browser.new_sections
    }

    /// Takes the count of new sections from the transcript, after an update
    /// that may have changed it (keys report it themselves, as a `Msg`).
    pub(super) fn sync_new_sections(&mut self) {
        let count = self
            .application
            .query(&self.top_transcript(), NEW_SECTIONS_ATTR)
            .ok()
            .flatten()
            .map(tuirealm::props::QueryResult::into_attr);
        if let Some(AttrValue::Length(count)) = count {
            self.set_new_sections(count);
        }
    }

    /// The transcript counts `count` new sections: shows `↓ N new` in the
    /// help line, or removes it at 0.
    pub(super) fn set_new_sections(&mut self, count: usize) {
        if count != self.browser.new_sections {
            self.browser.new_sections = count;
            self.refresh_status();
        }
    }

    /// Hands a turn update to the transcript at `depth`, which owns the turns.
    fn forward_to_transcript(&mut self, depth: usize, update: &SessionUpdate) {
        self.set_attr(
            transcript_id(depth),
            MODEL_UPDATE_ATTR,
            AttrValue::Payload(PropPayload::Any(Box::new(update.shared()))),
        );
    }

    /// Remembers the progress of the initial load and shows it.
    fn set_loading(&mut self, percent: Option<u32>) {
        self.browser.loading = percent;
        self.refresh_status();
    }

    /// Shows the status items in the help line: the progress of the load,
    /// then `↓ N new`, then the search position (`3/17`, SPEC §4.2).
    pub(super) fn refresh_status(&mut self) {
        let mut items = Vec::new();
        if let Some(percent) = self.browser.loading {
            items.push(format!("Loading… {percent}%"));
        }
        if self.browser.new_sections > 0 {
            items.push(format!("↓ {} new", self.browser.new_sections));
        }
        if let Some(position) = self.browser.search.position {
            items.push(position.to_string());
        }
        self.set_attr(Id::BrowserHelpLine, STATUS_ATTR, status_value(items));
    }

    /// Decides whether the session is empty: loaded, and the transcript
    /// holds no turn (asked of the transcript, which owns them).
    pub(super) fn refresh_session_empty(&mut self) {
        let turns = self
            .application
            .query(&self.top_transcript(), TURN_COUNT_ATTR)
            .ok()
            .flatten()
            .map(tuirealm::props::QueryResult::into_attr);
        let empty = self.browser.loaded && matches!(turns, Some(AttrValue::Length(0)));
        if empty != self.browser.empty {
            self.browser.empty = empty;
            self.request_redraw(RedrawRequest::Dirty);
        }
    }

    // -- the selector over the browser

    /// `Ctrl-o`: opens the selector over the browser with the open session
    /// selected and Sessions focused. Does nothing without an open session
    /// or when the selector is already open.
    pub(super) fn open_selector(&mut self) {
        if self.screen != Screen::Browser {
            return;
        }
        let Some(open) = self.browser.open.clone() else {
            return;
        };
        if let Err(err) = mount_selector(&mut self.application, &self.keymap, &self.theme, false) {
            log::error!("could not mount the selector: {err}");
            self.unmount_selector();
            return;
        }
        self.selector.show_session(open.harness, &open.summary);
        self.screen = Screen::BrowserSelector;
        self.push_all();
        self.push_viewports();
        self.set_session_open(true);
        let before = self.application.focus().cloned();
        match self.application.active(&Id::SessionList) {
            Ok(()) => self.focus_stack.extend(before),
            Err(err) => log::warn!("could not focus the sessions: {err}"),
        }
        self.sync_pane_focus();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// `Esc` in the selector: closes it and returns the focus to the
    /// component that had it (the transcript).
    pub(super) fn close_selector(&mut self) {
        if self.screen != Screen::BrowserSelector {
            return;
        }
        self.unmount_selector();
        self.screen = Screen::Browser;
        // The first entry is who had the focus before the selector; a prompt
        // that was open added more.
        let back = self
            .focus_stack
            .first()
            .copied()
            .filter(|id| self.application.mounted(id))
            .unwrap_or_else(|| self.top_transcript());
        self.focus_stack.clear();
        if let Err(err) = self.application.active(&back) {
            log::warn!("could not return the focus to {back:?}: {err}");
        }
        self.push_viewports();
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Unmounts the selector and its prompt, and ends its search. The
    /// selector's data stays.
    pub(super) fn unmount_selector(&mut self) {
        self.selector.search = None;
        for id in SELECTOR_IDS.into_iter().chain([Id::Prompt]) {
            if self.application.mounted(&id)
                && let Err(err) = self.application.umount(&id)
            {
                log::warn!("could not unmount {id:?}: {err}");
            }
        }
    }
}

/// The percentage `fraction` (0.0 to 1.0) stands for, rounded.
fn percent(fraction: f32) -> u32 {
    (fraction.clamp(0.0, 1.0) * 100.0).round() as u32
}
