//! The component that turns global keys, resizes and ticks into messages
//! (SPEC §1.4).

use ratatui::Frame;
use ratatui::layout::Rect;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;
use tuirealm::subscription::{EventClause, Sub, SubClause};

use crate::app::index_port::index_event_kind;
use crate::app::search_worker::search_event_kind;
use crate::app::session_port::session_event_kind;
use crate::app::{Id, Msg, UserEvent};
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
/// The quit and `open_selector` keys come from the keymap's `global` context;
/// `Ctrl-c` always quits. The other global keys (help, theme selector) are
/// added with the components they open.
#[derive(Debug, Clone)]
pub struct GlobalEvents {
    keymap: Keymap,
}

impl GlobalEvents {
    /// A component resolving keys with `keymap`.
    pub fn new(keymap: Keymap) -> Self {
        Self { keymap }
    }

    /// The subscriptions that make the application forward the events this
    /// component handles to it, whichever component has focus.
    ///
    /// Every key subscription uses [`key_clause`], so the global keys stop
    /// firing while a prompt or an overlay has focus, decided in one place.
    /// Resizes, ticks and worker events are never filtered by focus (see
    /// [`always_clause`]).
    pub fn subscriptions(keymap: &Keymap) -> Vec<Sub<Id, UserEvent>> {
        let mut subs: Vec<Sub<Id, UserEvent>> = quit_key_events(keymap)
            .into_iter()
            .chain(action_key_events(keymap, Action::OpenSelector))
            .map(|key| Sub::new(EventClause::Keyboard(key), key_clause()))
            .collect();
        subs.push(Sub::new(EventClause::WindowResize, always_clause()));
        subs.push(Sub::new(EventClause::Tick, always_clause()));
        for kind in [
            index_event_kind(),
            search_event_kind(),
            session_event_kind(),
        ] {
            subs.push(Sub::new(EventClause::Discriminant(kind), always_clause()));
        }
        subs
    }
}

/// When the global keys apply: not while the [`Prompt`](super::Prompt) has
/// focus. It is mounted exactly while it has focus (input mode, SPEC §6), and
/// handles its own keys, `Ctrl-c` among them. The overlays that take focus
/// later (help, theme selector) are added here.
fn key_clause() -> SubClause<Id> {
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

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

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
            Event::User(UserEvent::Index(update)) => Some(Msg::Index(update.clone())),
            Event::User(UserEvent::Search(update)) => Some(Msg::SearchResults(update.clone())),
            Event::User(UserEvent::Session { generation, update }) => Some(Msg::Session {
                generation: *generation,
                update: update.clone(),
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
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let subs = GlobalEvents::subscriptions(&Keymap::default());
        let keys = quit_key_events(&Keymap::default()).len();
        let open = action_key_events(&Keymap::default(), Action::OpenSelector);
        assert_eq!(open, [KeyEvent::new(Key::Char('o'), KeyModifiers::CONTROL)]);
        // Resize, tick, and the index, search and session events.
        assert_eq!(subs.len(), keys + open.len() + 5);
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
                    duration: std::time::Duration::ZERO,
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
