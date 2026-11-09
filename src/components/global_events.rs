//! The component that turns global keys, resizes and ticks into messages
//! (architecture.md#events-focus-and-keys).

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use tuirealm::subscription::{EventClause, MouseEventClause, Sub, SubClause};

use crate::app::index_port::index_event_kind;
use crate::app::search_worker::search_event_kind;
use crate::app::session_port::{session_event_kind, subagent_event_kind};
use crate::app::transcript_search::transcript_search_event_kind;
use crate::app::{Id, Msg, UserEvent};
use crate::components::{HELP_CONTEXT_ATTR, HelpContext, context_from_value};
use crate::keymap::{Action, Context, KeySpec, Keymap};

/// `Ctrl-c` always quits. It is not part of the keymap.
const CTRL_C: KeyEvent = KeyEvent {
    code: Key::Char('c'),
    modifiers: KeyModifiers::CONTROL,
};

/// Draws nothing. It is subscribed to the global keys, to resizes, to ticks
/// and to the user events of the session index, the search worker and the
/// open session, and turns them into messages.
///
/// The quit, help, theme and `open_selector` keys come from the keymap's
/// `global` context; `Ctrl-c` always quits.
///
/// The help key gives `Msg::OpenHelp` for the screen the app says is shown
/// (see [`HELP_CONTEXT_ATTR`]).
#[derive(Debug, Clone)]
pub struct GlobalEvents {
    keymap: Keymap,
    help_context: HelpContext,
}

impl GlobalEvents {
    /// A component resolving keys with `keymap`. The help it opens is the
    /// selector's, the screen the application starts on.
    pub fn new(keymap: Keymap) -> Self {
        Self {
            keymap,
            help_context: HelpContext::Selector,
        }
    }

    /// The subscriptions that make the application forward the events this
    /// component handles to it, whichever component has focus.
    ///
    /// Every key subscription uses [`key_clause`], so the global keys stop
    /// firing while a prompt or an overlay has focus, decided in one place.
    /// Resizes, ticks and worker events are never filtered by focus (see
    /// [`always_clause`]).
    ///
    /// With `mouse` it is also subscribed to the mouse anywhere on the
    /// screen and turns the left press and the wheel (see [`MOUSE_KINDS`])
    /// into [`Msg::Mouse`]; the `App` finds the component under the pointer.
    /// Without, no mouse event reaches it.
    pub fn subscriptions(keymap: &Keymap, mouse: bool) -> Vec<Sub<Id, UserEvent>> {
        let mut subs: Vec<Sub<Id, UserEvent>> = quit_key_events(keymap)
            .into_iter()
            .map(|key| {
                // `Ctrl-c` is the one global key an overlay lets through.
                let clause = if KeySpec::from_event(&key).is_ctrl_c() {
                    prompt_clause()
                } else {
                    key_clause()
                };
                Sub::new(EventClause::Keyboard(key), clause)
            })
            .chain(
                [Action::Help, Action::Theme, Action::OpenSelector]
                    .into_iter()
                    .flat_map(|action| action_key_events(keymap, action))
                    .map(|key| Sub::new(EventClause::Keyboard(key), key_clause())),
            )
            .collect();
        subs.push(Sub::new(EventClause::WindowResize, always_clause()));
        subs.push(Sub::new(EventClause::Tick, always_clause()));
        if mouse {
            // One subscription for every mouse event on the screen. The
            // library forwards on the position alone, whatever the kind and
            // modifiers of the clause, so `on` picks the events it acts on.
            subs.push(Sub::new(
                EventClause::Mouse(MouseEventClause {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    modifiers: KeyModifiers::NONE,
                    column: 0..u16::MAX,
                    row: 0..u16::MAX,
                }),
                always_clause(),
            ));
        }
        for kind in [
            index_event_kind(),
            search_event_kind(),
            transcript_search_event_kind(),
            session_event_kind(),
            subagent_event_kind(),
        ] {
            subs.push(Sub::new(EventClause::Discriminant(kind), always_clause()));
        }
        subs
    }
}

/// The mouse events the application acts on: a left press and a wheel step
/// either way, without modifiers.
pub const MOUSE_KINDS: [MouseEventKind; 3] = [
    MouseEventKind::Down(MouseButton::Left),
    MouseEventKind::ScrollUp,
    MouseEventKind::ScrollDown,
];

/// Whether the application acts on `mouse`: a left press or a wheel step,
/// without modifiers (`Shift` is the terminal's text selection).
fn is_acted_on(mouse: &MouseEvent) -> bool {
    mouse.modifiers == KeyModifiers::NONE && MOUSE_KINDS.contains(&mouse.kind)
}

/// When the global keys apply: not while the [`Prompt`](super::Prompt), the
/// help overlay, the details overlay or the theme selector has focus. Each is
/// mounted exactly while it has focus, and handles its own keys (the prompt,
/// `Ctrl-c` as well).
fn key_clause() -> SubClause<Id> {
    SubClause::and(
        prompt_clause(),
        SubClause::not(SubClause::or(
            SubClause::IsMounted(Id::HelpOverlay),
            SubClause::or(
                SubClause::IsMounted(Id::DetailsOverlay),
                SubClause::IsMounted(Id::ThemeSelector),
            ),
        )),
    )
}

/// When `Ctrl-c` applies: not while the prompt has focus, which handles it.
/// It quits from every overlay.
fn prompt_clause() -> SubClause<Id> {
    SubClause::not(SubClause::IsMounted(Id::Prompt))
}

/// When resizes, ticks and worker results apply: always. A prompt or an
/// overlay must not make the app miss a resize or a snapshot or stats (which
/// are not sent again).
fn always_clause() -> SubClause<Id> {
    SubClause::Always
}

/// The key events to subscribe to for quitting: `Ctrl-c` and every key bound
/// to `quit` in the `global` context.
///
/// Subscriptions match a key event exactly, but terminals report characters
/// such as `Q` or `?` with or without `Shift`, so for plain character keys
/// the `Shift` variant is included as well.
pub fn quit_key_events(keymap: &Keymap) -> Vec<KeyEvent> {
    let mut events = vec![CTRL_C];
    for event in action_key_events(keymap, Action::Quit) {
        if !events.contains(&event) {
            events.push(event);
        }
    }
    events
}

/// The key events to subscribe to for the `global` action `action`: every
/// single key bound to it (a chord cannot be subscribed to), with the `Shift`
/// variant of plain character keys.
pub fn action_key_events(keymap: &Keymap, action: Action) -> Vec<KeyEvent> {
    let mut events = Vec::new();
    for seq in keymap.keys(Context::Global, action) {
        if seq.is_chord() {
            continue;
        }
        let event = seq.first().to_event();
        let shifted = matches!(event.code, Key::Char(_)) && event.modifiers == KeyModifiers::NONE;
        for candidate in std::iter::once(event).chain(shifted.then_some(KeyEvent {
            code: event.code,
            modifiers: KeyModifiers::SHIFT,
        })) {
            if !events.contains(&candidate) {
                events.push(candidate);
            }
        }
    }
    events
}

impl Component for GlobalEvents {
    fn view(&mut self, _frame: &mut Frame, _area: Rect) {}

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        if attr == HELP_CONTEXT_ATTR
            && let Some(context) = context_from_value(&value)
        {
            self.help_context = context;
        }
    }

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for GlobalEvents {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.on_key(key),
            Event::WindowResize(width, height) => Some(Msg::Resized(*width, *height)),
            Event::Tick => Some(Msg::Tick),
            Event::Mouse(mouse) if is_acted_on(mouse) => Some(Msg::Mouse(*mouse)),
            Event::User(UserEvent::Index(update)) => Some(Msg::Index(update.clone())),
            Event::User(UserEvent::Search(update)) => Some(Msg::SearchResults(update.clone())),
            Event::User(UserEvent::TranscriptSearch(matches)) => {
                Some(Msg::TranscriptSearchResults(matches.clone()))
            }
            Event::User(UserEvent::Session { generation, update }) => Some(Msg::Session {
                generation: *generation,
                update: update.clone(),
            }),
            Event::User(UserEvent::Subagent { generation, found }) => Some(Msg::SubagentFound {
                generation: *generation,
                found: found.clone(),
            }),
            _ => None,
        }
    }
}

impl GlobalEvents {
    fn on_key(&self, key: &KeyEvent) -> Option<Msg> {
        if KeySpec::from_event(key).is_ctrl_c() {
            return Some(Msg::Quit);
        }
        match self.keymap.resolve(Context::Global, key)? {
            Action::Quit => Some(Msg::Quit),
            Action::OpenSelector => Some(Msg::OpenSelector),
            Action::Help => Some(Msg::OpenHelp(self.help_context)),
            Action::Theme => Some(Msg::OpenThemeSelector),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::context_value;
    use crate::config::RawKeys;

    fn key(code: Key, modifiers: KeyModifiers) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent { code, modifiers })
    }

    fn char_key(c: char) -> Event<UserEvent> {
        key(Key::Char(c), KeyModifiers::NONE)
    }

    fn ctrl(c: char) -> Event<UserEvent> {
        key(Key::Char(c), KeyModifiers::CONTROL)
    }

    fn with_quit_key(quit: &str) -> Keymap {
        let mut raw = RawKeys::new();
        raw.entry("global".to_owned())
            .or_default()
            .insert("quit".to_owned(), vec![quit.to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        keymap
    }

    #[test]
    fn every_default_quit_key_quits() {
        let mut component = GlobalEvents::new(Keymap::default());
        for event in [char_key('q'), ctrl('q'), ctrl('w'), ctrl('c')] {
            assert_eq!(component.on(&event), Some(Msg::Quit), "{event:?}");
        }
    }

    #[test]
    fn a_window_resize_becomes_resized() {
        let mut component = GlobalEvents::new(Keymap::default());
        assert_eq!(
            component.on(&Event::WindowResize(100, 30)),
            Some(Msg::Resized(100, 30))
        );
    }

    #[test]
    fn a_tick_becomes_tick() {
        let mut component = GlobalEvents::new(Keymap::default());
        assert_eq!(component.on(&Event::Tick), Some(Msg::Tick));
    }

    #[test]
    fn other_keys_and_events_produce_nothing() {
        let mut component = GlobalEvents::new(Keymap::default());
        for event in [
            char_key('a'),
            key(Key::Enter, KeyModifiers::NONE),
            ctrl('d'),
            Event::FocusGained,
            Event::None,
        ] {
            assert_eq!(component.on(&event), None, "{event:?}");
        }
    }

    #[test]
    fn a_remapped_quit_key_replaces_the_default() {
        let mut component = GlobalEvents::new(with_quit_key("x"));
        assert_eq!(component.on(&char_key('x')), Some(Msg::Quit));
        assert_eq!(component.on(&char_key('q')), None);
        assert_eq!(component.on(&ctrl('q')), None);
        assert_eq!(component.on(&ctrl('c')), Some(Msg::Quit));
    }

    #[test]
    fn default_quit_subscriptions_cover_the_default_keys_and_ctrl_c() {
        let events = quit_key_events(&Keymap::default());
        for expected in [
            KeyEvent::new(Key::Char('q'), KeyModifiers::NONE),
            KeyEvent::new(Key::Char('q'), KeyModifiers::CONTROL),
            KeyEvent::new(Key::Char('w'), KeyModifiers::CONTROL),
            KeyEvent::new(Key::Char('c'), KeyModifiers::CONTROL),
        ] {
            assert!(events.contains(&expected), "{expected:?} in {events:?}");
        }
    }

    #[test]
    fn subscriptions_follow_the_remapped_quit_key() {
        let events = quit_key_events(&with_quit_key("x"));
        assert!(events.contains(&KeyEvent::new(Key::Char('x'), KeyModifiers::NONE)));
        assert!(events.contains(&KeyEvent::new(Key::Char('c'), KeyModifiers::CONTROL)));
        assert!(!events.contains(&KeyEvent::new(Key::Char('q'), KeyModifiers::NONE)));
        assert!(!events.contains(&KeyEvent::new(Key::Char('q'), KeyModifiers::CONTROL)));
    }

    #[test]
    fn uppercase_quit_key_is_also_subscribed_with_shift() {
        let events = quit_key_events(&with_quit_key("Q"));
        assert!(events.contains(&KeyEvent::new(Key::Char('Q'), KeyModifiers::NONE)));
        assert!(events.contains(&KeyEvent::new(Key::Char('Q'), KeyModifiers::SHIFT)));
        let mut component = GlobalEvents::new(with_quit_key("Q"));
        assert_eq!(
            component.on(&key(Key::Char('Q'), KeyModifiers::SHIFT)),
            Some(Msg::Quit)
        );
    }

    #[test]
    fn subscriptions_include_the_keys_resize_tick_and_the_worker_events() {
        let subs = GlobalEvents::subscriptions(&Keymap::default(), false);
        let keys = quit_key_events(&Keymap::default()).len();
        let open = action_key_events(&Keymap::default(), Action::OpenSelector);
        assert_eq!(open, [KeyEvent::new(Key::Char('o'), KeyModifiers::CONTROL)]);
        let help = action_key_events(&Keymap::default(), Action::Help);
        assert_eq!(
            help,
            [
                KeyEvent::new(Key::Char('?'), KeyModifiers::NONE),
                KeyEvent::new(Key::Char('?'), KeyModifiers::SHIFT)
            ]
        );
        let theme = action_key_events(&Keymap::default(), Action::Theme);
        assert_eq!(
            theme,
            [
                KeyEvent::new(Key::Char('t'), KeyModifiers::NONE),
                KeyEvent::new(Key::Char('t'), KeyModifiers::SHIFT)
            ]
        );
        // Resize, tick, and the index, session search, transcript search,
        // session and subagent lookup events.
        assert_eq!(subs.len(), keys + open.len() + help.len() + theme.len() + 7);
    }

    fn mouse(kind: MouseEventKind, modifiers: KeyModifiers) -> Event<UserEvent> {
        Event::Mouse(MouseEvent {
            kind,
            modifiers,
            column: 7,
            row: 3,
        })
    }

    fn mouse_subscriptions(mouse: bool) -> usize {
        GlobalEvents::subscriptions(&Keymap::default(), mouse).len()
            - GlobalEvents::subscriptions(&Keymap::default(), false).len()
    }

    #[test]
    fn the_mouse_is_subscribed_to_only_with_mouse_on() {
        assert_eq!(mouse_subscriptions(false), 0);
        assert_eq!(mouse_subscriptions(true), 1);
    }

    #[test]
    fn presses_and_wheel_steps_are_given_to_the_app() {
        let mut component = GlobalEvents::new(Keymap::default());
        for kind in MOUSE_KINDS {
            let event = mouse(kind, KeyModifiers::NONE);
            let Event::Mouse(raw) = event.clone() else {
                unreachable!()
            };
            assert_eq!(component.on(&event), Some(Msg::Mouse(raw)), "{kind:?}");
        }
    }

    #[test]
    fn other_mouse_events_are_dropped() {
        let mut component = GlobalEvents::new(Keymap::default());
        for (kind, modifiers) in [
            (MouseEventKind::Down(MouseButton::Right), KeyModifiers::NONE),
            (
                MouseEventKind::Down(MouseButton::Middle),
                KeyModifiers::NONE,
            ),
            (MouseEventKind::Up(MouseButton::Left), KeyModifiers::NONE),
            (MouseEventKind::Drag(MouseButton::Left), KeyModifiers::NONE),
            (MouseEventKind::Moved, KeyModifiers::NONE),
            (MouseEventKind::ScrollLeft, KeyModifiers::NONE),
            (MouseEventKind::ScrollRight, KeyModifiers::NONE),
            (MouseEventKind::Down(MouseButton::Left), KeyModifiers::SHIFT),
            (MouseEventKind::ScrollDown, KeyModifiers::CONTROL),
        ] {
            assert_eq!(component.on(&mouse(kind, modifiers)), None, "{kind:?}");
        }
    }

    #[test]
    fn the_theme_key_opens_the_theme_selector() {
        let mut component = GlobalEvents::new(Keymap::default());
        assert_eq!(component.on(&char_key('t')), Some(Msg::OpenThemeSelector));
        // On both screens: the help context does not matter.
        component.attr(HELP_CONTEXT_ATTR, context_value(HelpContext::Browser));
        assert_eq!(component.on(&char_key('t')), Some(Msg::OpenThemeSelector));
    }

    #[test]
    fn a_remapped_theme_key_replaces_the_t() {
        let mut raw = RawKeys::new();
        raw.entry("global".to_owned())
            .or_default()
            .insert("theme".to_owned(), vec!["f2".to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut component = GlobalEvents::new(keymap);
        assert_eq!(
            component.on(&key(Key::Function(2), KeyModifiers::NONE)),
            Some(Msg::OpenThemeSelector)
        );
        assert_eq!(component.on(&char_key('t')), None);
    }

    #[test]
    fn the_help_key_opens_the_help_of_the_screen_the_app_reports() {
        let mut component = GlobalEvents::new(Keymap::default());
        let shifted = key(Key::Char('?'), KeyModifiers::SHIFT);
        assert_eq!(
            component.on(&char_key('?')),
            Some(Msg::OpenHelp(HelpContext::Selector))
        );
        component.attr(HELP_CONTEXT_ATTR, context_value(HelpContext::Browser));
        assert_eq!(
            component.on(&char_key('?')),
            Some(Msg::OpenHelp(HelpContext::Browser))
        );
        assert_eq!(
            component.on(&shifted),
            Some(Msg::OpenHelp(HelpContext::Browser))
        );
        component.attr(HELP_CONTEXT_ATTR, context_value(HelpContext::Selector));
        assert_eq!(
            component.on(&char_key('?')),
            Some(Msg::OpenHelp(HelpContext::Selector))
        );
    }

    #[test]
    fn a_remapped_help_key_replaces_the_question_mark() {
        let mut raw = RawKeys::new();
        raw.entry("global".to_owned())
            .or_default()
            .insert("help".to_owned(), vec!["f1".to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut component = GlobalEvents::new(keymap);
        assert!(matches!(
            component.on(&key(Key::Function(1), KeyModifiers::NONE)),
            Some(Msg::OpenHelp(_))
        ));
        assert_eq!(component.on(&char_key('?')), None);
    }

    #[test]
    fn ctrl_o_opens_the_selector() {
        let mut component = GlobalEvents::new(Keymap::default());
        assert_eq!(component.on(&ctrl('o')), Some(Msg::OpenSelector));
        assert_eq!(component.on(&char_key('o')), None);
    }

    #[test]
    fn a_remapped_open_selector_key_replaces_ctrl_o() {
        let mut raw = RawKeys::new();
        raw.entry("global".to_owned())
            .or_default()
            .insert("open_selector".to_owned(), vec!["ctrl-p".to_owned()]);
        let (keymap, warnings) = Keymap::from_config(&raw);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut component = GlobalEvents::new(keymap.clone());
        assert_eq!(component.on(&ctrl('p')), Some(Msg::OpenSelector));
        assert_eq!(component.on(&ctrl('o')), None);
        assert_eq!(
            action_key_events(&keymap, Action::OpenSelector),
            [KeyEvent::new(Key::Char('p'), KeyModifiers::CONTROL)]
        );
    }

    #[test]
    fn a_session_user_event_becomes_a_session_message() {
        use crate::app::SessionUpdate;
        use crate::model::ModelUpdate;
        let update = SessionUpdate::new(4, ModelUpdate::Loaded);
        let mut component = GlobalEvents::new(Keymap::default());
        let msg = component.on(&Event::User(UserEvent::Session {
            generation: 9,
            update: update.clone(),
        }));
        assert_eq!(
            msg,
            Some(Msg::Session {
                generation: 9,
                update
            })
        );
    }

    #[test]
    fn a_subagent_user_event_becomes_a_subagent_found_message() {
        use crate::model::SessionId;
        let found = Some(SessionId::from("s1/subagents/agent-a1"));
        let mut component = GlobalEvents::new(Keymap::default());
        let msg = component.on(&Event::User(UserEvent::Subagent {
            generation: 3,
            found: found.clone(),
        }));
        assert_eq!(
            msg,
            Some(Msg::SubagentFound {
                generation: 3,
                found
            })
        );
    }

    #[test]
    fn an_index_user_event_becomes_an_index_message() {
        use crate::app::IndexUpdate;
        use crate::index::IndexEvent;
        use crate::model::{HarnessId, SessionId, SessionStats};
        let update = IndexUpdate::new(
            7,
            IndexEvent::Stats {
                harness: HarnessId::Claude,
                session: SessionId::from("s"),
                stats: SessionStats {
                    turns: 1,
                    agent_time: std::time::Duration::ZERO,
                    run_time: std::time::Duration::ZERO,
                    tokens: crate::model::TokenUsage::default(),
                },
            },
        );
        let mut component = GlobalEvents::new(Keymap::default());
        let msg = component.on(&Event::User(UserEvent::Index(update.clone())));
        assert_eq!(msg, Some(Msg::Index(update)));
    }

    #[test]
    fn it_draws_nothing() {
        let mut component = GlobalEvents::new(Keymap::default());
        let buffer = crate::test_support::render(5, 2, |frame| {
            let area = frame.area();
            component.view(frame, area);
        });
        assert_eq!(
            crate::test_support::buffer_to_text(&buffer),
            "     \n     \n"
        );
    }
}
