//! Pointer routing (architecture.md#events-focus-and-keys as amended by the
//! cycle 5 spec, §0 item 6; product §5.9), and, until the components keep
//! it, the double-click state of the selector, the theme selector and the
//! turns sidebar.
//!
//! `GlobalEvents` turns every left press and wheel step into
//! [`Msg::Pointer`] (only when `mouse = true`). [`App::update`] asks
//! [`hit_test`] (in `layout.rs`, beside `layout`) where it goes, over the
//! routing layout: the pure [`layout`] function that `App::view` uses too,
//! cached in the `App` with its input and refreshed whenever the screen, an
//! overlay or the size changes, never from `view`. `hit_test` holds the
//! rules that depend on the screen: what an open overlay or prompt blocks,
//! and that the toast takes nothing.
//!
//! The component it finds gets a [`RoutedMouse`] (the event with the
//! position relative to the component's area) as [`UserEvent::Pointer`]; the
//! overlay on top gets a left press outside it as
//! [`UserEvent::PointerOutside`], and closes itself as on `Esc` unless it is
//! modal. The component alone decides what the event means. Its answer is
//! queued with `emit`, never handled by calling `update` again:
//! [`App::handle`] handles it before any later event.
//!
//! Why not hand the component the event itself? The application forwards
//! every raw `Event::Mouse` to the focused component, in screen coordinates,
//! and a component cannot tell those from routed ones.

use std::time::{Duration, Instant};

use ratatui::layout::{Rect, Size};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};

use super::layout::{LayoutInput, PointerTarget, ScreenLayout, hit_test, layout};
use super::{App, Id, Msg, Pane, UserEvent};
use crate::components::{SELECTED_TURN_ATTR, payload_value};
use crate::model::{ProjectId, SessionId, TurnId};

/// Two presses on the same row at most this far apart are a double-click.
pub const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// A mouse event for the component under the pointer: the position is
/// relative to the component's area (its top-left cell is 0, 0), and `area`
/// is that area's size. Built by the router; no Port delivers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoutedMouse {
    pub kind: MouseEventKind,
    pub column: u16,
    pub row: u16,
    pub area: Size,
}

impl RoutedMouse {
    /// `event` as seen from `area`, which must contain it.
    pub fn new(event: MouseEvent, area: Rect) -> Self {
        Self {
            kind: event.kind,
            column: event.column.saturating_sub(area.x),
            row: event.row.saturating_sub(area.y),
            area: area.as_size(),
        }
    }

    /// Whether this is the left button being pressed.
    pub fn is_left_press(&self) -> bool {
        self.kind == MouseEventKind::Down(MouseButton::Left)
    }
}

/// The last left press, for double-clicks.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ClickTarget {
    Project(ProjectId),
    Session(SessionId),
    Theme(String),
    Turn(TurnId),
}

/// What the `App` keeps for the mouse: the layout it routes with and the
/// input it was laid out from, the last press, and the clock the
/// double-click is measured with.
pub(super) struct MouseState {
    /// The areas events are routed with.
    pub(super) routing: ScreenLayout,
    /// What `routing` was laid out from.
    routing_input: LayoutInput,
    last_press: Option<(ClickTarget, Instant)>,
    /// Whether a click handler recorded the last left press as a press on a
    /// row.
    row_pressed: bool,
    clock: Box<dyn Fn() -> Instant>,
}

impl Default for MouseState {
    fn default() -> Self {
        Self {
            routing: ScreenLayout::default(),
            routing_input: LayoutInput::default(),
            last_press: None,
            row_pressed: false,
            clock: Box::new(Instant::now),
        }
    }
}

impl MouseState {
    pub(super) fn set_clock(&mut self, clock: impl Fn() -> Instant + 'static) {
        self.clock = Box::new(clock);
    }

    /// A new left press: a press before it that no click handler recorded
    /// was not on a row (a tab, empty space, anywhere else), and ends a
    /// double-click that was under way. The handlers only see the press once
    /// its component's answer is handled, after routing, so this is checked
    /// at the next press.
    fn left_press_started(&mut self) {
        if !self.row_pressed {
            self.last_press = None;
        }
        self.row_pressed = false;
    }

    /// Records a left press on `target`. Returns whether it completes a
    /// double-click: the previous press was on the same target at most
    /// [`DOUBLE_CLICK`] ago. A completed double-click is forgotten, so a
    /// third press starts a new one.
    fn press(&mut self, target: ClickTarget) -> bool {
        self.row_pressed = true;
        let now = (self.clock)();
        let double = self.last_press.as_ref().is_some_and(|(last, at)| {
            *last == target && now.saturating_duration_since(*at) <= DOUBLE_CLICK
        });
        self.last_press = (!double).then_some((target, now));
        double
    }
}

impl App {
    /// The areas mouse events are routed with.
    pub fn routing(&self) -> &ScreenLayout {
        &self.mouse.routing
    }

    /// Replaces the clock double-clicks are measured with (tests inject a
    /// fixed one).
    pub fn set_click_clock(&mut self, clock: impl Fn() -> Instant + 'static) {
        self.mouse.set_clock(clock);
    }

    /// Recomputes the routing areas from the current state. `update` calls
    /// it after every message (any of them can open an overlay or a prompt,
    /// or change the screen), `push_viewports` when the screen or the size
    /// changes, and `show_toast_for` when a toast appears; `view` never
    /// calls it. It is a handful of rectangles, so it is not worth finding
    /// out which messages can change the layout.
    pub(super) fn refresh_routing(&mut self) {
        let area = Rect::new(0, 0, self.size.0, self.size.1);
        let input = self.layout_input();
        self.mouse.routing = layout(&input, area);
        self.mouse.routing_input = input;
    }

    /// A left press or a wheel step at a position of the screen: gives it to
    /// the component [`hit_test`] finds, wherever the focus is, and queues
    /// its answer.
    pub(super) fn pointer(&mut self, event: MouseEvent) {
        if !self.config.mouse {
            return;
        }
        if event.kind == MouseEventKind::Down(MouseButton::Left) {
            self.mouse.left_press_started();
        }
        let (id, routed) = match hit_test(&self.mouse.routing, &self.mouse.routing_input, event) {
            // The layout places "the transcript"; the one on top gets it.
            PointerTarget::Component(Id::Transcript, area) => (
                self.top_transcript(),
                UserEvent::Pointer(RoutedMouse::new(event, area)),
            ),
            PointerTarget::Component(id, area) => {
                (id, UserEvent::Pointer(RoutedMouse::new(event, area)))
            }
            PointerTarget::Outside(overlay, _) => (overlay, UserEvent::PointerOutside),
            PointerTarget::Nothing => return,
        };
        let answer = self
            .application
            .get_component_mut(&id)
            .and_then(|component| component.on(&Event::User(routed)));
        if let Some(msg) = answer {
            self.emit(msg);
        }
    }

    // -- the theme selector's clicks

    /// A click on the theme `id`, where the theme selector's cursor is now: it
    /// is previewed. A double-click keeps it, like `Enter`.
    pub(super) fn theme_clicked(&mut self, id: &str) {
        if self.theme_selector.is_none() {
            return;
        }
        if self.mouse.press(ClickTarget::Theme(id.to_owned())) {
            self.select_theme(id);
        } else {
            self.preview_theme(id);
        }
    }

    // -- the turns sidebar's clicks

    /// A click on the entry of turn `id`: the sidebar gets the focus, the
    /// entry is selected and the transcript scrolls to its block. A
    /// double-click is `Enter` on it: the transcript gets the focus.
    pub(super) fn turn_clicked(&mut self, id: TurnId) {
        let double = self.mouse.press(ClickTarget::Turn(id.clone()));
        // Focus first: the sidebar takes its selection from the transcript
        // unless it has the focus itself.
        self.focus_turns();
        self.set_attr(
            Id::TurnSidebar,
            SELECTED_TURN_ATTR,
            payload_value(id.clone()),
        );
        self.go_to_turn(&id, double);
    }

    // -- the selector's clicks

    /// A click on a project row: Projects gets the focus and the project is
    /// selected. A double-click is `Enter` on it: Sessions gets the focus
    /// and, with none selected, its newest session.
    pub(super) fn project_clicked(&mut self, id: ProjectId) {
        let double = self.mouse.press(ClickTarget::Project(id.clone()));
        // Focus first: in the filtered selector it ends the filter, which
        // resets the project.
        self.focus_pane(Pane::Projects);
        self.project_selected(id);
        if double {
            self.focus_pane(Pane::Sessions);
        }
    }

    /// A click on a session row: Sessions gets the focus and the session is
    /// selected. A double-click opens it.
    pub(super) fn session_clicked(&mut self, id: SessionId) {
        let double = self.mouse.press(ClickTarget::Session(id.clone()));
        // Select first: focusing Sessions with nothing selected would select
        // the newest session, which resets the scroll of the list and moves
        // the clicked row from under the pointer.
        self.session_selected(id.clone());
        self.focus_pane(Pane::Sessions);
        if double && let Some(harness) = self.selector.current {
            self.emit(Msg::OpenSession(harness, id));
        }
    }
}
