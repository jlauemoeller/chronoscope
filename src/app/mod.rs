//! The application: the tui-realm [`Application`] plus the domain state, and
//! the three types that connect them (architecture.md#application-structure).
//!
//! - [`Id`] names a mountable component.
//! - [`Msg`] says what happened, in application terms (all but
//!   [`Msg::Pointer`], which exists for the pointer routing only): the
//!   global messages, and one nested enum per concern, which its handler
//!   module defines and dispatches.
//! - [`UserEvent`] is what the Ports deliver, and the pointer events the
//!   router hands to components ([`UserEvent::Pointer`],
//!   [`UserEvent::PointerOutside`], [`UserEvent::PointerElsewhere`]).
//!
//! [`App::update`] is the only place that changes domain state or the mounted
//! components. [`App::view`] only asks [`layout`] where each component goes and
//! calls [`Application::view`] for it.

pub mod browser;
mod copy;
pub mod details;
pub mod help;
pub mod hints;
pub mod index_port;
pub mod layout;
pub mod mouse;
mod output;
mod overlay;
mod prompt;
pub mod redraw;
pub mod resume;
pub mod runtime;
pub mod save;
pub mod search;
pub mod search_worker;
pub mod selector;
mod session_panels;
pub mod session_port;
mod sidebar;
pub mod startup;
pub mod theme_selector;
mod transcript;
pub mod transcript_search;
mod worker;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use ratatui::Frame;
use tuirealm::application::{Application, ApplicationError, PollStrategy};
use tuirealm::component::AppComponent;
use tuirealm::event::MouseEvent;
use tuirealm::listener::EventListenerCfg;
use tuirealm::subscription::{EventClause, Sub, SubClause};

use crate::clipboard::{Clipboard, NoClipboard};
use crate::components::turn_block::SectionId;
use crate::components::{EmptyState, GlobalEvents, TOAST_DURATION, TOO_SMALL, Toast};
use crate::config::{Config, ConfigWarning};
use crate::harness::Harness;
use crate::index::{IndexEvent, IndexHandle};
use crate::keymap::Keymap;
use crate::model::{HarnessId, SessionId};
use crate::theme::{TerminalEnv, Theme, ThemeRegistry};
use crate::timestamp::{DisplayTz, TimestampFormatter};
use crate::transcript_search::SearchMatches;
pub use browser::BrowserMsg;
use browser::BrowserState;
pub use index_port::{IndexPort, IndexUpdate};
use layout::{LayoutInput, Screen, layout};
pub use mouse::RoutedMouse;
pub use output::OutputMsg;
pub use overlay::OverlayMsg;
pub use prompt::{PromptMsg, PromptPurpose};
use redraw::RedrawRequest;
pub use runtime::Startup;
pub use search::{Hit, search_sessions};
pub use search_worker::{SearchPort, SearchResult, SearchUpdate, SearchWorker};
use selector::SelectorState;
pub use selector::{Pane, SelectorMsg};
pub use session_port::{SessionEvent, SessionPort, SessionUpdate, session_channel};
pub use startup::AppInit;
pub use theme_selector::ThemeMsg;
pub use transcript::TranscriptMsg;
pub use transcript_search::{
    TranscriptSearchPort, TranscriptSearchWorker, transcript_search_event_kind,
};
use unicode_width::UnicodeWidthStr;
pub use worker::WorkerMsg;

/// One variant per mountable component (architecture.md#components).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    /// Turns global keys, resizes and ticks into messages.
    GlobalEvents,
    /// The whole screen under the selector when the selector is the base
    /// screen: paints its background, takes no mouse.
    ScreenBackground,
    /// The selector overlay's border, sizing and background.
    SelectorFrame,
    /// "Terminal too small" (and, later, other empty states).
    EmptyState,
    /// The harness tabs of the selector.
    HarnessTabs,
    /// The Projects pane of the selector.
    ProjectList,
    /// The Sessions pane of the selector.
    SessionList,
    /// The `Session Time` panel below the Sessions pane.
    SessionTime,
    /// The `Session Tokens` panel below `Session Time`.
    SessionTokens,
    /// The Working Directory box of the selector.
    WorkingDir,
    /// Key hints of the selector.
    HelpLine,
    /// Project and session titles, working directory and parse warnings of
    /// the browser.
    BrowserHeader,
    /// The turns and compaction events of the transcript on top, in a pane
    /// to the left of it.
    TurnSidebar,
    /// The column between the turns sidebar and the transcript: paints its
    /// background, takes no mouse.
    SidebarGap,
    /// The open session's turns.
    Transcript,
    /// The turns of the subagent opened at this depth (1 for the first, 2 for
    /// one opened from it, ...). The transcripts below it stay mounted but
    /// are not drawn (architecture.md#components).
    Subagent(usize),
    /// Key hints and status items of the browser.
    BrowserHelpLine,
    /// "Session file is empty", in place of the transcript.
    EmptySession,
    /// The search, path and `y/n` prompt, drawn in place of the help line.
    Prompt,
    /// The keybindings overlay.
    HelpOverlay,
    /// The metadata of the selected section.
    DetailsOverlay,
    /// The theme list with live preview.
    ThemeSelector,
    /// The notification in the bottom-right corner.
    Toast,
}

/// What happened, in application terms. Never raw keys.
///
/// The top level holds what is global; everything else is in the nested
/// enum of its concern, which [`App::update`] hands to that concern's
/// handler module (architecture.md#application-structure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// `run_app` starts the application with what the command line resolved
    /// to: the startup warnings' toast, then the preselection, the lists the
    /// command line was resolved against, and the session to open or the
    /// sessions to list. Sent once, before the loop runs; boxed, as it is
    /// much larger than any other message.
    Start(Box<Startup>),
    /// Leave the application.
    Quit,
    /// The terminal now has this many columns and rows.
    Resized(u16, u16),
    /// A tick of the event loop.
    Tick,
    /// The toast with this id has been shown for its duration.
    ToastExpired(u64),
    /// Something happened in the session selector.
    Selector(SelectorMsg),
    /// Something happened to the open session, its subagents or the turns
    /// sidebar.
    Browser(BrowserMsg),
    /// The transcript on top reported a change of its view or asked for a
    /// search.
    Transcript(TranscriptMsg),
    /// Something happened to the help or details overlay.
    Overlay(OverlayMsg),
    /// Something happened in the theme selector.
    Theme(ThemeMsg),
    /// The help-line prompt reported an edit or an answer.
    Prompt(PromptMsg),
    /// The user asked to copy, save, export or resume.
    Output(OutputMsg),
    /// A worker thread answered.
    Worker(WorkerMsg),
    /// A left press or a wheel step at this position of the screen (only
    /// with `mouse = true`), in screen coordinates. It is built on the
    /// router's path (cycle 5 spec, §0 item 6), not by a Port: `GlobalEvents`
    /// makes it from the raw event, and the router gives it to the component
    /// that [`hit_test`](layout::hit_test) finds.
    ///
    /// It is the one `Msg` that is not in application terms: it exists for
    /// that routing only, and what it means is the component's to decide.
    Pointer(MouseEvent),
}

/// What an event did to the transcript's view, as its answer reports it.
/// The transcript owns its selection and its count of new sections, and
/// reports here what a key, a click or the wheel changed of them, so the App
/// never asks (cycle 5 spec, §9.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ViewChange {
    /// Whether anything the transcript shows changed (the selection, the
    /// scroll or the folds): a redraw is needed.
    pub changed: bool,
    /// The count of new sections, when the event changed it.
    pub new_sections: Option<usize>,
    /// The section selected now, when the event moved the selection. A
    /// selection that went to none (every section hidden) is not reported:
    /// the turns sidebar keeps the entry it shows, as it does for none.
    pub selection: Option<SectionId>,
}

impl ViewChange {
    /// The message a key or a wheel step that made this change answers:
    /// [`TranscriptMsg::NewSections`] when the count changed (with the selection, if
    /// it moved too), else [`TranscriptMsg::SelectionChanged`] when the selection
    /// moved, else [`TranscriptMsg::Changed`] when anything else changed,
    /// else none.
    pub fn key_answer(self) -> Option<Msg> {
        if let Some(count) = self.new_sections {
            return Some(Msg::Transcript(TranscriptMsg::NewSections {
                count,
                selection: self.selection,
            }));
        }
        if let Some(section) = self.selection {
            return Some(Msg::Transcript(TranscriptMsg::SelectionChanged(section)));
        }
        self.changed
            .then_some(Msg::Transcript(TranscriptMsg::Changed))
    }
}

/// What the Ports deliver to the application. Must be `Eq + Clone`, so large
/// payloads are `Arc`-wrapped and compared by a sequence number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {
    /// The session index worker reported a new snapshot or stats.
    Index(IndexUpdate),
    /// The search worker finished a search.
    Search(SearchUpdate),
    /// The transcript search worker finished a search.
    TranscriptSearch(SearchMatches),
    /// The harness reported something about the session being loaded;
    /// `generation` numbers the load.
    Session {
        generation: u64,
        update: SessionUpdate,
    },
    /// The lookup of a subagent's transcript answered; `generation` is the
    /// number its load gets, and `found` the subagent's session, if any.
    Subagent {
        generation: u64,
        found: Option<SessionId>,
    },
    /// A pointer event the router gave the component under the pointer,
    /// with the position relative to the component's area. Built by the
    /// router (cycle 5 spec, §0 item 6), never delivered by a Port: the raw
    /// `Event::Mouse` goes to the focused component too, in screen
    /// coordinates, and components ignore it.
    Pointer(RoutedMouse),
    /// A left press outside the overlay on top, which the router gives to
    /// that overlay: it closes as on `Esc`, unless it is modal. Built by the
    /// router, never delivered by a Port.
    PointerOutside,
    /// A left press went somewhere else than the component that got the last
    /// one: on another component, outside an overlay, or on nothing. The
    /// router gives it to that component before it routes the press, so a
    /// double-click under way there ends; the component forgets its last
    /// press and answers nothing. Built by the router (cycle 5 spec, §0 item
    /// 6, §8.3), never delivered by a Port.
    PointerElsewhere,
}

/// How often the event listener sends `Event::Tick`.
pub const TICK_INTERVAL: Duration = Duration::from_millis(250);
/// How often the input port checks for key and mouse events.
pub const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How long one loop iteration waits for an event.
pub const POLL_TIMEOUT: Duration = Duration::from_millis(20);
/// The most events one loop iteration handles.
const MAX_EVENTS_PER_TICK: usize = 64;

/// The event listener configuration of the real application: crossterm input,
/// a tick, and, when the session index runs, its events.
pub fn production_listener(
    index: Option<Receiver<IndexEvent>>,
    search: Option<Receiver<SearchResult>>,
    transcript_search: Option<Receiver<SearchMatches>>,
    session: Option<Receiver<SessionEvent>>,
) -> EventListenerCfg<UserEvent> {
    let cfg = EventListenerCfg::default()
        .crossterm_input_listener(INPUT_POLL_INTERVAL, 10)
        .tick_interval(TICK_INTERVAL);
    let cfg = match session {
        Some(events) => cfg.add_port(
            Box::new(SessionPort::new(events)),
            session_port::SESSION_POLL_INTERVAL,
            session_port::SESSION_MAX_PER_POLL,
        ),
        None => cfg,
    };
    let cfg = match search {
        Some(results) => cfg.add_port(
            Box::new(SearchPort::new(results)),
            search_worker::SEARCH_POLL_INTERVAL,
            search_worker::SEARCH_MAX_PER_POLL,
        ),
        None => cfg,
    };
    let cfg = match transcript_search {
        Some(results) => cfg.add_port(
            Box::new(TranscriptSearchPort::new(results)),
            transcript_search::TRANSCRIPT_SEARCH_POLL_INTERVAL,
            transcript_search::TRANSCRIPT_SEARCH_MAX_PER_POLL,
        ),
        None => cfg,
    };
    match index {
        Some(events) => cfg.add_port(
            Box::new(IndexPort::new(events)),
            index_port::INDEX_POLL_INTERVAL,
            index_port::INDEX_MAX_PER_POLL,
        ),
        None => cfg,
    }
}

/// The application: the component tree and the domain state.
pub struct App {
    application: Application<Id, Msg, UserEvent>,
    config: Config,
    registry: ThemeRegistry,
    theme: Theme,
    keymap: Keymap,
    warnings: Vec<ConfigWarning>,
    screen: Screen,
    toast_text: Option<String>,
    toast_id: u64,
    size: (u16, u16),
    quit: bool,
    redraw: RedrawRequest,
    selector: SelectorState,
    timestamps: TimestampFormatter,
    clock: Box<dyn Fn() -> DateTime<Utc>>,
    index: Option<IndexHandle>,
    /// Who had the focus before each overlay or prompt that is open, oldest
    /// first (architecture.md#events-focus-and-keys).
    focus_stack: Vec<Id>,
    /// The thread that runs the searches; without one they run on the UI
    /// thread (tests).
    search_worker: Option<SearchWorker>,
    /// Numbers the search requests.
    search_seq: u64,
    /// The thread that searches the transcript; without one the search runs
    /// on the UI thread (tests).
    transcript_worker: Option<TranscriptSearchWorker>,
    /// Numbers the transcript searches. Never reset, so an answer cannot be
    /// taken for one to a search of another session.
    transcript_seq: u64,
    /// Messages `update` made for itself, the browser or a component's
    /// answer to a pointer event, oldest first; [`App::handle`] handles
    /// them, and `tick` hands out any that are left with the components'.
    queued: VecDeque<Msg>,
    /// The harnesses sessions can be loaded from.
    harnesses: HashMap<HarnessId, Arc<dyn Harness>>,
    /// Where the forwarding threads of session loads send their updates.
    session_sink: Option<Sender<SessionEvent>>,
    /// The open session.
    browser: BrowserState,
    /// Where copied text goes (architecture.md#application-structure).
    clipboard: Box<dyn Clipboard>,
    /// Where OSC52 sequences are written: the terminal. `None` when there is
    /// no terminal to write to.
    osc52_out: Option<Box<dyn std::io::Write>>,
    /// The size the help overlay wants (borders included) while it is open.
    help_size: Option<ratatui::layout::Size>,
    /// The details overlay's content and the size it wants (borders
    /// included), while it is open.
    details: Option<details::DetailsSizing>,
    /// The theme selector: what it wants and what to restore on `Esc`, while
    /// it is open.
    theme_selector: Option<theme_selector::ThemeSelection>,
    /// What the pointer router keeps: the routing areas, which component got
    /// the last left press, and the clock it hands the components.
    mouse: mouse::PointerState,
    /// The id of the active theme.
    theme_id: String,
    /// The terminal the theme is adapted to, as at startup.
    theme_env: TerminalEnv,
    /// The config file the chosen theme is saved to; none when the home
    /// directory is unknown.
    config_path: Option<PathBuf>,
    /// The directory relative paths of saves and exports are under: the one
    /// the application started in.
    save_dir: PathBuf,
    /// What `~` stands for in those paths.
    home_dir: Option<PathBuf>,
    /// The resume waiting for its `y/n` answer, and the one confirmed.
    resume: resume::ResumeState,
}

impl App {
    /// Builds the application and mounts its components.
    ///
    /// `listener` configures the event listener: [`production_listener`] for
    /// the real application, or one without ports for tests. `working_dir`
    /// is the directory the user started in (`std::env::current_dir()`); the
    /// selector starts on the project that contains it (product §3.1).
    pub fn new(
        init: AppInit,
        listener: EventListenerCfg<UserEvent>,
        working_dir: PathBuf,
    ) -> Result<Self, ApplicationError> {
        let AppInit {
            config,
            registry,
            theme,
            keymap,
            warnings,
            harnesses,
            theme_id,
            env,
            config_path,
        } = init;
        let mut application = Application::init(listener);
        application.mount(
            Id::GlobalEvents,
            Box::new(GlobalEvents::new(keymap.clone())),
            GlobalEvents::subscriptions(&keymap, config.mouse),
        )?;
        let mouse = mouse::PointerState::default();
        selector::mount_selector(&mut application, &keymap, &theme, mouse.click_clock(), true)?;
        application.mount(
            Id::EmptyState,
            Box::new(EmptyState::new(TOO_SMALL, theme.clone())),
            Vec::new(),
        )?;
        // The selector starts on the Projects pane.
        application.active(&Id::ProjectList)?;
        let timestamps =
            TimestampFormatter::new(&config.timestamp_format, config.utc, DisplayTz::Local);
        let mut app = Self {
            application,
            config,
            registry,
            theme,
            keymap,
            warnings,
            screen: Screen::default(),
            toast_text: None,
            toast_id: 0,
            size: (0, 0),
            quit: false,
            redraw: RedrawRequest::Dirty,
            selector: SelectorState::new(harnesses, working_dir.clone()),
            timestamps,
            clock: Box::new(Utc::now),
            index: None,
            focus_stack: Vec::new(),
            search_worker: None,
            search_seq: 0,
            transcript_worker: None,
            transcript_seq: 0,
            queued: VecDeque::new(),
            harnesses: HashMap::new(),
            session_sink: None,
            browser: BrowserState::default(),
            clipboard: Box::new(NoClipboard),
            osc52_out: None,
            help_size: None,
            details: None,
            theme_selector: None,
            mouse,
            theme_id,
            theme_env: env,
            config_path,
            save_dir: working_dir,
            home_dir: crate::paths::Paths::detect().map(|paths| paths.home_dir),
            resume: resume::ResumeState::default(),
        };
        app.push_tabs();
        Ok(app)
    }

    /// Replaces the clock the `●` marker is computed with (tests inject a
    /// fixed time).
    pub fn set_clock(&mut self, clock: impl Fn() -> DateTime<Utc> + 'static) {
        self.clock = Box::new(clock);
    }

    /// Hands the app the session index worker it displays. The app owns it
    /// from now on; [`shutdown`](Self::shutdown) stops it.
    pub fn set_index(&mut self, index: IndexHandle) {
        self.index = Some(index);
    }

    /// Stops the search worker, and the session index worker, and waits until
    /// it has saved its cache.
    pub fn shutdown(&mut self) {
        self.browser.stop_watching();
        if let Some(mut worker) = self.search_worker.take() {
            worker.shutdown();
        }
        if let Some(mut worker) = self.transcript_worker.take() {
            worker.shutdown();
        }
        if let Some(index) = self.index.take() {
            index.shutdown();
        }
    }

    /// Shows `text` as a toast for [`TOAST_DURATION`], replacing the toast on
    /// screen, if any.
    fn show_toast(&mut self, text: impl Into<String>) -> Result<(), ApplicationError> {
        self.show_toast_for(text, TOAST_DURATION)
    }

    /// Shows `text` as a toast that expires after `duration`.
    fn show_toast_for(
        &mut self,
        text: impl Into<String>,
        duration: Duration,
    ) -> Result<(), ApplicationError> {
        let text = text.into();
        self.toast_id += 1;
        let toast = Toast::new(text.clone(), self.theme.clone(), Instant::now(), duration)
            .with_id(self.toast_id);
        let subs = vec![Sub::new(EventClause::Tick, SubClause::Always)];
        self.application.remount(Id::Toast, Box::new(toast), subs)?;
        self.toast_text = Some(text);
        self.refresh_routing();
        self.request_redraw(RedrawRequest::Dirty);
        Ok(())
    }

    /// The text of the toast on screen, if any.
    pub fn toast_text(&self) -> Option<&str> {
        self.toast_text.as_deref()
    }

    // -- domain state

    /// The loaded configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// The available themes.
    pub fn theme_registry(&self) -> &ThemeRegistry {
        &self.registry
    }

    /// The active theme.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The active keymap.
    pub fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    /// The warnings collected at startup (config, theme and keymap), to be
    /// shown as a toast.
    pub fn warnings(&self) -> &[ConfigWarning] {
        &self.warnings
    }

    /// The last known terminal size as `(columns, rows)`.
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// Whether the application should exit.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// The mounted components, for tests and, later, focus handling.
    pub fn application(&self) -> &Application<Id, Msg, UserEvent> {
        &self.application
    }

    /// The mounted components, mutably: tests move the focus to components
    /// no key reaches yet.
    #[cfg(test)]
    pub(crate) fn application_mut(&mut self) -> &mut Application<Id, Msg, UserEvent> {
        &mut self.application
    }

    /// Returns what the screen needs and resets it to nothing.
    pub fn take_redraw(&mut self) -> RedrawRequest {
        std::mem::take(&mut self.redraw)
    }

    // -- event loop

    /// Polls the components' event sources once and returns the messages the
    /// components produced.
    ///
    /// The messages that `update` queued and [`handle`](Self::handle) has
    /// not handled come after them.
    pub fn tick(&mut self) -> Result<Vec<Msg>, ApplicationError> {
        let mut msgs = self
            .application
            .tick(PollStrategy::UpTo(MAX_EVENTS_PER_TICK, POLL_TIMEOUT))?;
        msgs.extend(self.queued.drain(..));
        Ok(msgs)
    }

    /// The run loop's step: [`update`](Self::update)s `msg`, then each
    /// message that queued, in the order they were queued, until none is
    /// left. A pointer event's answer is thus handled before any later
    /// event. Never called from inside `update`.
    pub fn handle(&mut self, msg: Msg) {
        self.update(msg);
        while let Some(next) = self.queued.pop_front() {
            self.update(next);
        }
    }

    /// Queues `msg` to be handled after the current one: [`update`](Self::update)
    /// returns nothing and never calls itself, so this is how it passes a
    /// request on (to the browser, for the sessions the selector opens) and
    /// how the router passes on a component's answer to a pointer event.
    fn emit(&mut self, msg: Msg) {
        self.queued.push_back(msg);
    }

    /// Takes the messages that `update` queued and that neither
    /// [`handle`](Self::handle) nor `tick` has handed out yet.
    pub fn take_queued(&mut self) -> Vec<Msg> {
        self.queued.drain(..).collect()
    }

    // -- focus

    /// Mounts the overlay or prompt `id` and gives it the focus; the
    /// component that had it is remembered for [`close_overlay`](Self::close_overlay)
    /// (architecture.md#events-focus-and-keys).
    fn open_overlay(
        &mut self,
        id: Id,
        component: Box<dyn AppComponent<Msg, UserEvent>>,
    ) -> Result<(), ApplicationError> {
        self.application.mount(id, component, Vec::new())?;
        let before = self.application.focus().cloned();
        if let Err(err) = self.application.active(&id) {
            self.application.umount(&id)?;
            return Err(err);
        }
        self.focus_stack.extend(before);
        self.request_redraw(RedrawRequest::Dirty);
        Ok(())
    }

    /// Unmounts the overlay or prompt `id` and returns the focus to the
    /// component that had it when it was opened.
    fn close_overlay(&mut self, id: Id) {
        if let Err(err) = self.application.umount(&id) {
            log::warn!("could not unmount {id:?}: {err}");
        }
        if let Some(before) = self.focus_stack.pop()
            && let Err(err) = self.application.active(&before)
        {
            log::warn!("could not return the focus to {before:?}: {err}");
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    /// Applies `msg`: the only place that changes domain state or components.
    /// It hands each message to the dispatcher of its concern.
    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Start(startup) => self.start(*startup),
            Msg::Quit => self.quit(),
            Msg::Resized(width, height) => self.resized(width, height),
            Msg::Tick => self.tick_elapsed(),
            Msg::ToastExpired(id) => self.toast_expired(id),
            Msg::Selector(msg) => self.selector_msg(msg),
            Msg::Browser(msg) => self.browser_msg(msg),
            Msg::Transcript(msg) => self.transcript_msg(msg),
            Msg::Overlay(msg) => self.overlay_msg(msg),
            Msg::Theme(msg) => self.theme_msg(msg),
            Msg::Prompt(msg) => self.prompt_msg(msg),
            Msg::Output(msg) => self.output_msg(msg),
            Msg::Worker(msg) => self.worker_msg(msg),
            Msg::Pointer(event) => self.pointer(event),
        }
        // Any message may have moved the focus, which decides whether the
        // turns sidebar follows the transcript's selection.
        self.sync_sidebar_selection();
        self.refresh_hints();
        // Any message may have selected another session or brought its stats.
        self.push_session_panels();
        // Any message may have changed the screen, an overlay or the prompt.
        self.refresh_routing();
    }

    fn quit(&mut self) {
        self.quit = true;
    }

    fn resized(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.push_viewports();
        // A sidebar that is now hidden cannot keep the focus.
        self.release_hidden_sidebar();
        self.request_redraw(RedrawRequest::Resized);
    }

    fn tick_elapsed(&mut self) {
        // The toast expires itself on `Tick` (see `Toast`).
        self.refresh_activity();
        self.refresh_live();
    }

    /// Removes the toast, unless `id` is that of a toast that was replaced.
    fn toast_expired(&mut self, id: u64) {
        if id != self.toast_id || self.toast_text.take().is_none() {
            return;
        }
        if let Err(err) = self.application.umount(&Id::Toast) {
            log::warn!("could not unmount the toast: {err}");
        }
        self.request_redraw(RedrawRequest::Dirty);
    }

    fn request_redraw(&mut self, request: RedrawRequest) {
        self.redraw = self.redraw.max(request);
    }

    // -- drawing

    /// The layout input for the current state.
    fn layout_input(&self) -> LayoutInput {
        LayoutInput {
            screen: self.screen,
            toast_text_width: self.toast_text.as_deref().map(UnicodeWidthStr::width),
            prompt: self.application.mounted(&Id::Prompt),
            selector_search: self.selector.search.is_some(),
            session_empty: self.browser.session_empty(),
            help: self.help_size,
            details: self.details.as_ref().map(|d| d.size),
            theme_selector: self.theme_selector.as_ref().map(|s| s.size),
        }
    }

    /// Draws every visible component, overlays last. Draws nothing itself.
    pub fn view(&mut self, frame: &mut Frame) {
        let screen_layout = layout(&self.layout_input(), frame.area());
        let transcript = self.top_transcript();
        for &(id, area) in screen_layout.areas() {
            // The layout places "the transcript"; the one on top is drawn.
            let id = if id == Id::Transcript { transcript } else { id };
            self.application.view(&id, frame, area);
        }
    }
}
