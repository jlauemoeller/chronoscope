//! The application: the tui-realm [`Application`] plus the domain state, and
//! the three types that connect them (SPEC §1.2).
//!
//! - [`Id`] names a mountable component.
//! - [`Msg`] says what happened, in application terms.
//! - [`UserEvent`] is what the Ports deliver.
//!
//! [`App::update`] is the only place that changes domain state or the mounted
//! components. [`App::view`] only asks [`layout`] where each component goes and
//! calls [`Application::view`] for it.

pub mod browser;
mod copy;
pub mod help;
pub mod hints;
pub mod index_port;
pub mod layout;
pub mod redraw;
pub mod runtime;
pub mod save;
pub mod search;
pub mod search_worker;
pub mod selector;
pub mod session_port;
pub mod startup;
pub mod theme_selector;
pub mod transcript_search;

#[cfg(test)]
mod browser_tests;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod search_tests;
#[cfg(test)]
mod selection_tests;
#[cfg(test)]
mod selector_tests;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use ratatui::Frame;
use tuirealm::application::{Application, ApplicationError, PollStrategy};
use tuirealm::command::Direction;
use tuirealm::component::AppComponent;
use tuirealm::listener::EventListenerCfg;
use tuirealm::subscription::{EventClause, Sub, SubClause};

use crate::clipboard::{Clipboard, NoClipboard};
use crate::components::{EmptyState, GlobalEvents, HelpContext, TOAST_DURATION, TOO_SMALL, Toast};
use crate::config::{Config, ConfigWarning};
use crate::harness::Harness;
use crate::index::{IndexEvent, IndexHandle};
use crate::keymap::Keymap;
use crate::model::{HarnessId, SessionId, ToolCallId};
use crate::theme::{TerminalEnv, Theme, ThemeRegistry};
use crate::timestamp::{DisplayTz, TimestampFormatter};
use crate::transcript_search::{SearchCorpus, SearchMatches, SearchPosition};
use browser::BrowserState;
use hints::startup_toast_text;
pub use index_port::{IndexPort, IndexUpdate};
use layout::{LayoutInput, Screen, layout};
use redraw::RedrawRequest;
pub use search::{Hit, search_sessions};
pub use search_worker::{SearchPort, SearchResult, SearchUpdate, SearchWorker};
use selector::SelectorState;
pub use selector::{Pane, SelectorMsg};
pub use session_port::{SessionEvent, SessionPort, SessionUpdate, session_channel};
pub use startup::AppInit;
pub use transcript_search::{
    TranscriptSearchPort, TranscriptSearchWorker, transcript_search_event_kind,
};
use unicode_width::UnicodeWidthStr;

/// One variant per mountable component (SPEC §1.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Id {
    /// Turns global keys, resizes and ticks into messages.
    GlobalEvents,
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
    /// The Working Directory box of the selector.
    WorkingDir,
    /// Key hints of the selector.
    HelpLine,
    /// Project and session titles, working directory and parse warnings of
    /// the browser.
    BrowserHeader,
    /// The open session's turns.
    Transcript,
    /// The turns of the subagent opened at this depth (1 for the first, 2 for
    /// one opened from it, ...). The transcripts below it stay mounted but
    /// are not drawn (SPEC §1.3).
    Subagent(usize),
    /// Key hints and status items of the browser.
    BrowserHelpLine,
    /// "Session file is empty", in place of the transcript.
    EmptySession,
    /// The search, path and `y/n` prompt, drawn in place of the help line.
    Prompt,
    /// The keybindings overlay.
    HelpOverlay,
    /// The theme list with live preview.
    ThemeSelector,
    /// The notification in the bottom-right corner.
    Toast,
}

/// What happened, in application terms. Never raw keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// Leave the application.
    Quit,
    /// The help key: open the keybindings overlay for this screen.
    OpenHelp(HelpContext),
    /// `Esc` or the help key in the overlay: close it and return the focus.
    CloseHelp,
    /// A key scrolled the help overlay: nothing but a redraw is needed.
    HelpScrolled,
    /// The theme key: open the theme selector.
    OpenThemeSelector,
    /// The cursor of the theme selector moved to the theme with this id:
    /// show it as a preview.
    PreviewTheme(String),
    /// `Enter` in the theme selector on the theme with this id: keep it,
    /// save it to the config and close the selector.
    SelectTheme(String),
    /// `Esc` in the theme selector: restore the theme that was active when
    /// it opened and close it.
    CancelTheme,
    /// The terminal now has this many columns and rows.
    Resized(u16, u16),
    /// A tick of the event loop.
    Tick,
    /// The toast with this id has been shown for its duration.
    ToastExpired(u64),
    /// The session index worker reported something.
    Index(IndexUpdate),
    /// The search worker answered a search.
    SearchResults(SearchUpdate),
    /// Something happened in the session selector.
    Selector(SelectorMsg),
    /// The user asked to open this session of this harness.
    OpenSession(HarnessId, SessionId),
    /// The user asked for the session selector over the open session.
    OpenSelector,
    /// `Esc` in the selector with a session open behind it: close the
    /// selector.
    CloseSelector,
    /// The load of the open session reported something. `generation` is the
    /// number of the load; updates of a load that has been replaced are
    /// dropped.
    Session {
        generation: u64,
        update: SessionUpdate,
    },
    /// The text of the prompt changed.
    PromptChanged(String),
    /// Only the prompt's cursor moved: nothing but a redraw is needed.
    PromptMoved,
    /// A key changed what the transcript shows (selection, scroll, folds):
    /// nothing but a redraw is needed.
    TranscriptChanged,
    /// The transcript counts this many sections that arrived while the user
    /// was away from the end (`↓ N new`); 0 once following resumes. Keys
    /// report it this way; after the updates and viewport changes it
    /// forwards, the App reads the count from the transcript itself.
    NewSections(usize),
    /// `Enter` in the prompt, with its text (`"y"` for a yes in a `y/n`
    /// prompt).
    PromptSubmit(String),
    /// `Esc` in the prompt, or a no in a `y/n` prompt.
    PromptCancel,
    /// `↑` or `↓` in the prompt, for its owner to use.
    PromptNav(Direction),
    /// `/` in the transcript: open the search prompt in the help line.
    OpenSearch,
    /// The transcript asks for this search to be run on a snapshot of its
    /// turns. `generation` numbers the search; answers to any other are
    /// dropped.
    StartTranscriptSearch {
        query: String,
        corpus: SearchCorpus,
        generation: u64,
    },
    /// The worker answered a search of the transcript.
    TranscriptSearchResults(SearchMatches),
    /// A key moved among the matches (`3/17`) or cleared the search (`None`).
    SearchPosition(Option<SearchPosition>),
    /// The transcript asks for this text to be put on the clipboard: the
    /// selected section as displayed (`y`) or in full (`Y`).
    CopyText(String),
    /// `a` on a subagent section: open the transcript of the subagent that
    /// tool call `call_id` started. `label` (the agent type) is the
    /// breadcrumb's segment.
    OpenSubagent { call_id: ToolCallId, label: String },
    /// The back key in a subagent's transcript: return to the one it was
    /// opened from.
    CloseSubagent,
    /// The transcript asks for the selected section's text to be saved to a
    /// file (`w`, or `W` for its full content). `kind` is the word for the
    /// section in the default file name (`shell`, `user`, ...).
    SaveSection { text: String, kind: &'static str },
    /// The transcript asks for the session to be exported to a markdown file
    /// (`e` as currently folded, `E` in full, as `full` says). `document` is
    /// the markdown.
    Export { full: bool, document: String },
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
    /// first (SPEC §1.4).
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
    /// Messages `update` made for itself or for the browser; `tick` hands
    /// them out with the components'.
    queued: Vec<Msg>,
    /// The harnesses sessions can be loaded from.
    harnesses: HashMap<HarnessId, Arc<dyn Harness>>,
    /// Where the forwarding threads of session loads send their updates.
    session_sink: Option<Sender<SessionEvent>>,
    /// The open session.
    browser: BrowserState,
    /// Where copied text goes (SPEC §1.2).
    clipboard: Box<dyn Clipboard>,
    /// Where OSC52 sequences are written: the terminal. `None` when there is
    /// no terminal to write to.
    osc52_out: Option<Box<dyn std::io::Write>>,
    /// The size the help overlay wants (borders included) while it is open.
    help_size: Option<ratatui::layout::Size>,
    /// The theme selector: what it wants and what to restore on `Esc`, while
    /// it is open.
    theme_selector: Option<theme_selector::ThemeSelection>,
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
}

impl App {
    /// Builds the application and mounts its components.
    ///
    /// `listener` configures the event listener: [`production_listener`] for
    /// the real application, or one without ports for tests. `working_dir`
    /// is the directory the user started in (`std::env::current_dir()`); the
    /// selector starts on the project that contains it (SPEC §4.1).
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
            GlobalEvents::subscriptions(&keymap),
        )?;
        selector::mount_selector(&mut application, &keymap, &theme, true)?;
        application.mount(
            Id::EmptyState,
            Box::new(EmptyState::new(TOO_SMALL, theme.clone())),
            Vec::new(),
        )?;
        // The selector starts on the Projects pane.
        application.active(&Id::ProjectList)?;
        let toast_text = startup_toast_text(&warnings);
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
            queued: Vec::new(),
            harnesses: HashMap::new(),
            session_sink: None,
            browser: BrowserState::default(),
            clipboard: Box::new(NoClipboard),
            osc52_out: None,
            help_size: None,
            theme_selector: None,
            theme_id,
            theme_env: env,
            config_path,
            save_dir: working_dir,
            home_dir: crate::paths::Paths::detect().map(|paths| paths.home_dir),
        };
        app.push_tabs();
        // The startup warnings are shown once, here, and never again.
        if let Some(text) = toast_text {
            app.show_toast(text)?;
        }
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
    pub fn show_toast(&mut self, text: impl Into<String>) -> Result<(), ApplicationError> {
        self.show_toast_for(text, TOAST_DURATION)
    }

    /// Shows `text` as a toast that expires after `duration`.
    pub fn show_toast_for(
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

    /// Returns what the screen needs and resets it to nothing.
    pub fn take_redraw(&mut self) -> RedrawRequest {
        std::mem::take(&mut self.redraw)
    }

    // -- event loop

    /// Polls the components' event sources once and returns the messages the
    /// components produced.
    ///
    /// The messages that `update` queued come after them.
    pub fn tick(&mut self) -> Result<Vec<Msg>, ApplicationError> {
        let mut msgs = self
            .application
            .tick(PollStrategy::UpTo(MAX_EVENTS_PER_TICK, POLL_TIMEOUT))?;
        msgs.append(&mut self.queued);
        Ok(msgs)
    }

    /// Queues `msg` to be handled after the current one: [`update`](Self::update)
    /// returns nothing, so this is how it passes a request on (to the
    /// browser, for the sessions the selector opens).
    fn emit(&mut self, msg: Msg) {
        self.queued.push(msg);
    }

    /// Takes the messages that `update` queued and `tick` has not handed
    /// out yet.
    pub fn take_queued(&mut self) -> Vec<Msg> {
        std::mem::take(&mut self.queued)
    }

    // -- focus

    /// Mounts the overlay or prompt `id` and gives it the focus; the
    /// component that had it is remembered for [`close_overlay`](Self::close_overlay)
    /// (SPEC §1.4).
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
    /// It dispatches to one small handler per message.
    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Quit => self.quit(),
            Msg::OpenHelp(context) => self.open_help(context),
            Msg::CloseHelp => self.close_help(),
            Msg::HelpScrolled => self.request_redraw(RedrawRequest::Dirty),
            Msg::OpenThemeSelector => self.open_theme_selector(),
            Msg::PreviewTheme(id) => self.preview_theme(&id),
            Msg::SelectTheme(id) => self.select_theme(&id),
            Msg::CancelTheme => self.cancel_theme(),
            Msg::Resized(width, height) => self.resized(width, height),
            Msg::Tick => self.tick_elapsed(),
            Msg::ToastExpired(id) => self.toast_expired(id),
            Msg::Index(update) => self.index_update(&update),
            Msg::SearchResults(update) => self.search_result_arrived(&update),
            Msg::Selector(msg) => self.selector_msg(msg),
            Msg::PromptChanged(_) if self.browser.write.is_some() => {
                self.request_redraw(RedrawRequest::Dirty);
            }
            Msg::PromptSubmit(answer) if self.browser.write.is_some() => {
                self.write_prompt_submit(&answer);
            }
            Msg::PromptCancel if self.browser.write.is_some() => self.write_prompt_cancel(),
            Msg::PromptNav(_) if self.browser.write.is_some() => {}
            Msg::PromptChanged(_) if self.browser.search.prompt_open => {
                self.transcript_prompt_changed();
            }
            Msg::PromptChanged(text) => self.search_changed(text),
            Msg::PromptMoved | Msg::TranscriptChanged => self.request_redraw(RedrawRequest::Dirty),
            Msg::NewSections(count) => {
                // The key that reported it moved the view too.
                self.request_redraw(RedrawRequest::Dirty);
                self.set_new_sections(count);
            }
            Msg::PromptSubmit(text) if self.browser.search.prompt_open => {
                self.transcript_prompt_submit(text);
            }
            Msg::PromptSubmit(_) => self.search_submit(),
            Msg::PromptCancel if self.browser.search.prompt_open => {
                self.transcript_prompt_cancel();
            }
            Msg::PromptCancel => self.search_cancel(),
            Msg::PromptNav(_) if self.browser.search.prompt_open => {}
            Msg::PromptNav(direction) => self.search_nav(direction),
            Msg::OpenSearch => self.open_transcript_search(),
            Msg::StartTranscriptSearch {
                query,
                corpus,
                generation,
            } => self.start_transcript_search(query, corpus, generation),
            Msg::TranscriptSearchResults(matches) => self.transcript_matches_arrived(&matches),
            Msg::SearchPosition(position) => self.search_position_changed(position),
            Msg::CopyText(text) => self.copy_text(&text),
            Msg::SaveSection { text, kind } => self.save_section(text, kind),
            Msg::Export { full: _, document } => self.export_session(document),
            Msg::OpenSubagent { call_id, label } => self.open_subagent(&call_id, label),
            Msg::CloseSubagent => self.close_subagent(),
            Msg::OpenSession(harness, session) => self.open_session(harness, &session),
            Msg::OpenSelector => self.open_selector(),
            Msg::CloseSelector => self.close_selector(),
            Msg::Session { generation, update } => self.session_update(generation, &update),
        }
    }

    fn quit(&mut self) {
        self.quit = true;
    }

    fn resized(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.push_viewports();
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
            session_empty: self.browser.session_empty(),
            help: self.help_size,
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
