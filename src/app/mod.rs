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

pub mod hints;
pub mod layout;
pub mod redraw;
pub mod runtime;
pub mod startup;

#[cfg(test)]
mod tests;

use std::time::{Duration, Instant};

use ratatui::Frame;
use tuirealm::application::{Application, ApplicationError, PollStrategy};
use tuirealm::listener::EventListenerCfg;
use tuirealm::subscription::{EventClause, Sub, SubClause};

use crate::components::{
    EmptyState, GlobalEvents, HelpLine, SelectorFrame, TOAST_DURATION, TOO_SMALL, Toast,
};
use crate::config::{Config, ConfigWarning};
use crate::keymap::Keymap;
use crate::theme::{Role, Theme, ThemeRegistry};
use hints::{selector_hints, startup_toast_text};
use layout::{LayoutInput, Screen, layout};
use redraw::RedrawRequest;
pub use startup::AppInit;
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
    /// Key hints and status items.
    HelpLine,
    /// The notification in the bottom-right corner.
    Toast,
}

/// What happened, in application terms. Never raw keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Msg {
    /// Leave the application.
    Quit,
    /// The terminal now has this many columns and rows.
    Resized(u16, u16),
    /// A tick of the event loop.
    Tick,
    /// The toast with this id has been shown for its duration.
    ToastExpired(u64),
}

/// What the Ports deliver to the application. Must be `Eq + Clone`, so large
/// payloads are `Arc`-wrapped and compared by a sequence number.
///
/// There are no variants yet; the first ones come with the session index
/// worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserEvent {}

/// How often the event listener sends `Event::Tick`.
pub const TICK_INTERVAL: Duration = Duration::from_millis(250);
/// How often the input port checks for key and mouse events.
pub const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How long one loop iteration waits for an event.
pub const POLL_TIMEOUT: Duration = Duration::from_millis(20);
/// The most events one loop iteration handles.
const MAX_EVENTS_PER_TICK: usize = 64;

/// The event listener configuration of the real application: crossterm input
/// and a tick.
pub fn production_listener() -> EventListenerCfg<UserEvent> {
    EventListenerCfg::default()
        .crossterm_input_listener(INPUT_POLL_INTERVAL, 10)
        .tick_interval(TICK_INTERVAL)
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
}

impl App {
    /// Builds the application and mounts its components.
    ///
    /// `listener` configures the event listener: [`production_listener`] for
    /// the real application, or one without ports for tests.
    pub fn new(
        init: AppInit,
        listener: EventListenerCfg<UserEvent>,
    ) -> Result<Self, ApplicationError> {
        let AppInit {
            config,
            registry,
            theme,
            keymap,
            warnings,
        } = init;
        let mut application = Application::init(listener);
        application.mount(
            Id::GlobalEvents,
            Box::new(GlobalEvents::new(keymap.clone())),
            GlobalEvents::subscriptions(&keymap),
        )?;
        application.mount(
            Id::SelectorFrame,
            Box::new(SelectorFrame::new(theme.clone())),
            Vec::new(),
        )?;
        application.mount(
            Id::EmptyState,
            Box::new(EmptyState::new(TOO_SMALL, theme.clone())),
            Vec::new(),
        )?;
        application.mount(
            Id::HelpLine,
            Box::new(HelpLine::new(
                selector_hints(&keymap),
                Vec::new(),
                theme.clone(),
                Role::Surface,
            )),
            Vec::new(),
        )?;
        let toast_text = startup_toast_text(&warnings);
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
        };
        // The startup warnings are shown once, here, and never again.
        if let Some(text) = toast_text {
            app.show_toast(text)?;
        }
        Ok(app)
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
    pub fn tick(&mut self) -> Result<Vec<Msg>, ApplicationError> {
        self.application
            .tick(PollStrategy::UpTo(MAX_EVENTS_PER_TICK, POLL_TIMEOUT))
    }

    /// Applies `msg`: the only place that changes domain state or components.
    /// It dispatches to one small handler per message.
    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Quit => self.quit(),
            Msg::Resized(width, height) => self.resized(width, height),
            Msg::Tick => self.tick_elapsed(),
            Msg::ToastExpired(id) => self.toast_expired(id),
        }
    }

    fn quit(&mut self) {
        self.quit = true;
    }

    fn resized(&mut self, width: u16, height: u16) {
        self.size = (width, height);
        self.request_redraw(RedrawRequest::Resized);
    }

    fn tick_elapsed(&mut self) {
        // The live marker and activity flags come with the components that
        // need them. The toast expires itself on `Tick` (see `Toast`).
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
        }
    }

    /// Draws every visible component, overlays last. Draws nothing itself.
    pub fn view(&mut self, frame: &mut Frame) {
        let screen_layout = layout(&self.layout_input(), frame.area());
        for &(id, area) in screen_layout.areas() {
            self.application.view(&id, frame, area);
        }
    }
}
